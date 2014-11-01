// The MIT License (MIT)

// Copyright (c) 2014 Y. T. CHUNG

// Permission is hereby granted, free of charge, to any person obtaining a copy of
// this software and associated documentation files (the "Software"), to deal in
// the Software without restriction, including without limitation the rights to
// use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software is furnished to do so,
// subject to the following conditions:

// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
// FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
// COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
// IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
// CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//! TcpRelay server that running on local environment

use std::sync::{Arc, Mutex};
use std::io::{Listener, TcpListener, Acceptor, TcpStream};
use std::io::{
    EndOfFile,
    ConnectionFailed,
    ConnectionRefused,
    ConnectionReset,
    ConnectionAborted,
    BrokenPipe
};
use std::io::net::ip::Port;
use std::io::net::ip::{Ipv4Addr, Ipv6Addr};

use config::Config;

use relay::Relay;
use relay::socks5::parse_request_header;
use relay::tcprelay::{send_error_reply, relay_and_map};
use relay::socks5::{SOCKS5_VERSION, SOCKS5_AUTH_METHOD_NONE};
use relay::socks5::{SOCKS5_CMD_TCP_CONNECT, SOCKS5_CMD_TCP_BIND, SOCKS5_CMD_UDP_ASSOCIATE};
use relay::socks5::{SOCKS5_ADDR_TYPE_IPV6, SOCKS5_ADDR_TYPE_IPV4};
use relay::socks5::{
    SOCKS5_REPLY_COMMAND_NOT_SUPPORTED,
    SOCKS5_REPLY_HOST_UNREACHABLE,
    SOCKS5_REPLY_NETWORK_UNREACHABLE,
    SOCKS5_REPLY_GENERAL_FAILURE
};
use relay::socks5::SOCKS5_REPLY_SUCCEEDED;
use relay::loadbalancing::server::{LoadBalancer, RoundRobin};

use crypto::cipher;
use crypto::cipher::Cipher;

#[deriving(Clone)]
pub struct TcpRelayLocal {
    config: Config,
}

impl TcpRelayLocal {
    pub fn new(c: Config) -> TcpRelayLocal {
        if c.server.is_none() || c.local.is_none() {
            panic!("You have to provide configuration for server and local");
        }
        TcpRelayLocal {
            config: c,
        }
    }

    fn do_handshake(stream: &mut TcpStream) {
        // Read the handshake header
        // +----+----------+----------+
        // |VER | NMETHODS | METHODS  |
        // +----+----------+----------+
        // | 5  |    1     | 1 to 255 |
        // +----+----------+----------+
        let handshake_header = stream.read_exact(2).ok().expect("Error occurs while receiving handshake header");
        let (sock_ver, nmethods) = (handshake_header[0], handshake_header[1]);

        if sock_ver != SOCKS5_VERSION {
            // FIXME: Sometimes Chrome would send a header with version 0x50
            send_error_reply(stream, SOCKS5_REPLY_GENERAL_FAILURE);
            panic!("Invalid socks version \"{:x}\" in handshake", sock_ver);
        }

        let _ = stream.read_exact(nmethods as uint).ok().expect("Error occurs while receiving methods");
        // TODO: validating methods

        // Reply to client
        // +----+--------+
        // |VER | METHOD |
        // +----+--------+
        // | 1  |   1    |
        // +----+--------+
        let data_to_send: &[u8] = [SOCKS5_VERSION, SOCKS5_AUTH_METHOD_NONE];
        stream.write(data_to_send).ok().expect("Error occurs while sending handshake reply");
    }

    #[allow(dead_code)]
    fn handle_udp_associate_local(stream: &mut TcpStream) {
        let sockname = stream.socket_name().ok().expect("Failed to get socket name");
        let mut reply = vec![SOCKS5_VERSION, SOCKS5_REPLY_SUCCEEDED, 0x00,
                        SOCKS5_CMD_UDP_ASSOCIATE];
        match sockname.ip {
            Ipv4Addr(v1, v2, v3, v4) => {
                let ip = [v1, v2, v3, v4];
                reply.push(SOCKS5_ADDR_TYPE_IPV4);
                reply.push_all(ip)
            },
            Ipv6Addr(v1, v2, v3, v4, v5, v6, v7, v8) => {
                let ip = [(v1 >> 8) as u8, (v1 & 0xff) as u8,
                 (v2 >> 8) as u8, (v2 & 0xff) as u8,
                 (v3 >> 8) as u8, (v3 & 0xff) as u8,
                 (v4 >> 8) as u8, (v4 & 0xff) as u8,
                 (v5 >> 8) as u8, (v5 & 0xff) as u8,
                 (v6 >> 8) as u8, (v6 & 0xff) as u8,
                 (v7 >> 8) as u8, (v7 & 0xff) as u8,
                 (v8 >> 8) as u8, (v8 & 0xff) as u8];
                reply.push(SOCKS5_ADDR_TYPE_IPV6);
                reply.push_all(ip);
            }
        }

        reply.push((sockname.port >> 8) as u8);
        reply.push((sockname.port & 0xff) as u8);

        stream.write(reply.as_slice()).ok().expect("Failed to write to local stream");
    }

    fn handle_client(mut stream: TcpStream,
                     server_addr: String, server_port: Port,
                     password: String, encrypt_method: String) {
        TcpRelayLocal::do_handshake(&mut stream);

        let raw_header_part1 = stream.read_exact(3).ok().expect("Failed to read header");
        let (sock_ver, cmd) = (raw_header_part1[0], raw_header_part1[1]);

        if sock_ver != SOCKS5_VERSION {
            // FIXME: Sometimes Chrome would send a header with version 0x50
            send_error_reply(&mut stream, SOCKS5_REPLY_GENERAL_FAILURE);
            panic!("Invalid socks version \"{:x}\" in header", sock_ver);
        }

        let (header, addr) = {
            let mut header_buf = [0u8, .. 512];
            stream.read_at_least(1, header_buf).unwrap_or_else(|err| {
                send_error_reply(&mut stream, SOCKS5_REPLY_GENERAL_FAILURE);
                panic!("Error occurs while reading header: {}", err);
            });

            let (header_len, addr) = parse_request_header(header_buf).unwrap_or_else(|err| {
                send_error_reply(&mut stream, err.code);
                panic!("Error occurs while parsing request header: {}", err);
            });
            (header_buf.slice_to(header_len).to_vec(), addr)
        };

        let mut remote_stream = TcpStream::connect(server_addr.as_slice(),
                                           server_port).unwrap_or_else(|err| {
            match err.kind {
                ConnectionAborted | ConnectionReset | ConnectionRefused | ConnectionFailed => {
                    send_error_reply(&mut stream, SOCKS5_REPLY_HOST_UNREACHABLE);
                },
                _ => {
                    send_error_reply(&mut stream, SOCKS5_REPLY_NETWORK_UNREACHABLE);
                }
            }
            panic!("Failed to connect remote server: {}", err);
        });

        let mut cipher = cipher::with_name(encrypt_method.as_slice(),
                                       password.as_slice().as_bytes())
                                .expect("Unsupported cipher");

        match cmd {
            SOCKS5_CMD_TCP_CONNECT => {
                info!("CONNECT {}", addr);

                {
                    let reply = [SOCKS5_VERSION, SOCKS5_REPLY_SUCCEEDED,
                                    0x00, SOCKS5_CMD_TCP_CONNECT, 0x00, 0x00, 0x00, 0x00, 0x10, 0x10];
                    stream.write(reply)
                            .ok().expect("Error occurs while writing header to local stream");

                    let encrypted_header = cipher.encrypt(header.as_slice());
                    remote_stream.write(encrypted_header.as_slice())
                            .ok().expect("Error occurs while writing header to remote stream");
                }

                let mut remote_local_stream = stream.clone();
                let mut remote_remote_stream = remote_stream.clone();
                let mut remote_cipher = cipher.clone();
                let remote_addr_clone = addr.clone();
                spawn(proc() {
                    relay_and_map(&mut remote_remote_stream, &mut remote_local_stream,
                                  |msg| remote_cipher.decrypt(msg))
                        .unwrap_or_else(|err| {
                            match err.kind {
                                EndOfFile | BrokenPipe => {
                                    debug!("{} relay from remote to local stream: {}", remote_addr_clone, err)
                                },
                                _ => {
                                    error!("{} relay from remote to local stream: {}", remote_addr_clone, err)
                                }
                            }
                            remote_local_stream.close_write().or(Ok(())).unwrap();
                            remote_remote_stream.close_read().or(Ok(())).unwrap();
                        })
                });

                spawn(proc() {
                    relay_and_map(&mut stream, &mut remote_stream, |msg| cipher.encrypt(msg))
                        .unwrap_or_else(|err| {
                            match err.kind {
                                EndOfFile | BrokenPipe => {
                                    debug!("{} relay from local to remote stream: {}", addr, err)
                                },
                                _ => {
                                    error!("{} relay from local to remote stream: {}", addr, err)
                                }
                            }
                            remote_stream.close_write().or(Ok(())).unwrap();
                            stream.close_read().or(Ok(())).unwrap();
                        })
                });
            },
            SOCKS5_CMD_TCP_BIND => {
                warn!("BIND is not supported");
                send_error_reply(&mut stream, SOCKS5_REPLY_COMMAND_NOT_SUPPORTED);
            },
            SOCKS5_CMD_UDP_ASSOCIATE => {
                info!("UDP ASSOCIATE {}", addr);
                warn!("UDP ASSOCIATE is not supported");
                send_error_reply(&mut stream, SOCKS5_REPLY_COMMAND_NOT_SUPPORTED);

                // TcpRelayLocal::handle_udp_associate_local(stream);
            },
            _ => {
                // unsupported CMD
                send_error_reply(&mut stream, SOCKS5_REPLY_COMMAND_NOT_SUPPORTED);
                warn!("Unsupported command {}", cmd);
            }
        }
    }
}

impl Relay for TcpRelayLocal {
    fn run(&self) {
        let server_load_balancer = Arc::new(
                                        Mutex::new(
                                            RoundRobin::new(
                                                self.config.server.clone().expect("`server` should not be None"))));

        let local_conf = self.config.local.unwrap();

        let mut acceptor = match TcpListener::bind(local_conf.ip.to_string().as_slice(), local_conf.port).listen() {
            Ok(acpt) => acpt,
            Err(e) => {
                panic!("Error occurs while listening local address: {}", e.to_string());
            }
        };

        info!("Shadowsocks listening on {}", local_conf);

        loop {
            match acceptor.accept() {
                Ok(stream) => {
                    let (server_addr, server_port, password, encrypt_method) = {
                        let mut slb = server_load_balancer.lock();
                        let ref s = slb.pick_server();
                        (s.address.clone(), s.port.clone(), s.password.clone(), s.method.clone())
                    };

                    spawn(proc()
                        TcpRelayLocal::handle_client(stream,
                                                     server_addr, server_port,
                                                     password, encrypt_method));
                },
                Err(e) => {
                    panic!("Error occurs while accepting: {}", e.to_string());
                }
            }
        }
    }
}
