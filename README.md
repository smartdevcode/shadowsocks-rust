# shadowsocks-rust

[![Build Status](https://travis-ci.org/zonyitoo/shadowsocks-rust.svg)](https://travis-ci.org/zonyitoo/shadowsocks-rust)

This is a port of [shadowsocks](https://github.com/clowwindy/shadowsocks).

shadowsocks is a fast tunnel proxy that helps you bypass firewalls.

*Currently developing and testing with rust-nightly*

## Dependences

* libcrypto (OpenSSL)
* Rust nightly
* Cargo

## Usage

Build with [Cargo](http://doc.crates.io):

```bash
cargo build
```

Then `sslocal` and `ssserver` will appear in `./target`, it works similarly as the two binaries of
the official shadowsocks' implementation.

Enable more crypto algorithms by passing the name `cipher-[name]` via command line argument `--features`

```bash
cargo build --features "cipher-aes-ctr"
```

## Getting Started

Create a shadowsocks' configuration file. Example

```json
{
    "server": "my_server_ip",
    "server_port": 8388,
    "local_address": "127.0.0.1",
    "local_port": 1080,
    "password": "mypassword",
    "timeout": 300,
    "method": "aes-256-cfb",
    "fast_open": false,
}
```

Detailed explaination could be found in [shadowsocks' documentation](https://github.com/clowwindy/shadowsocks/wiki).

In shadowsocks-rust, we also have a extended configuration file format, which is able to define more than one server:

```json
{
    "servers": [
        {
            "address": "127.0.0.1",
            "port": 1080,
            "password": "hellofuck",
            "method": "bf-cfb",
            "timeout": 300,
        },
        {
            "address": "127.0.0.1",
            "port": 1081,
            "password": "hellofuck",
            "method": "aes-128-cfb"
        }
    ],
    "local_port": 8388,
    "local_address":"127.0.0.1",
    "fast_open": false
}
```

The `sslocal` will use a load balancing algorithm to dispatch packages to all servers.

Start local and server shadowsocks with

```
sslocal -c config.json
ssserver -c config.json
```

List all available arguments with `-h`.

Default log level is `error`, override it by setting environment variable `RUST_LOG`. Please refer
to [log crate](http://doc.rust-lang.org/log/index.html) for more detail.

## Notes

Still under developing and waiting for the final release of rust-1.0.

Currently implementation can only be built by rust-0.12-dev. It supports the following features:

* CONNECT command
* Crypto algorithms defined in `Cargo.toml`
* **Load balancing**

## TODO

* Documentation
* UDP_ASSOCIATION command
* BIND command
* Sock5 authentication
* <del>Extend configuration format</del>
* Fully testing on server
* Multiple worker
* User management

## License

[The MIT License (MIT)](http://opensource.org/licenses/MIT)

Copyright (c) 2014 Y. T. CHUNG

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
