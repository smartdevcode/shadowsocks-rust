//! Server side

use std::io;

use futures::{stream::futures_unordered, Future, Stream};

use config::Config;
use context::{Context, SharedContext};
use plugin::{launch_plugin, PluginMode};
use relay::{boxed_future, tcprelay::server::run as run_tcp, udprelay::server::run as run_udp};

/// Relay server running on server side.
///
/// ```no_run
/// extern crate tokio;
/// extern crate shadowsocks;
///
/// use shadowsocks::{
///     config::{Config, ServerConfig},
///     crypto::CipherType,
///     relay::server::run,
/// };
///
/// use tokio::prelude::*;
///
/// let mut config = Config::new();
/// config.server = vec![ServerConfig::basic(
///     "127.0.0.1:8388".parse().unwrap(),
///     "server-password".to_string(),
///     CipherType::Aes256Cfb,
/// )];
///
/// let fut = run(config);
/// tokio::run(fut.map_err(|err| panic!("Server run failed with error {}", err)));
/// ```
pub fn run(config: Config) -> impl Future<Item = (), Error = io::Error> + Send {
    futures::lazy(move || {
        let mut context = Context::new(config);

        let mut vf = Vec::new();

        if context.config().mode.enable_udp() {
            // Clone config here, because the config for TCP relay will be modified
            // after plugins started
            let udp_context = SharedContext::new(context.clone());

            // Run UDP relay before starting plugins
            // Because plugins doesn't support UDP relay
            let udp_fut = run_udp(udp_context);
            vf.push(boxed_future(udp_fut));
        }

        if context.config().mode.enable_tcp() {
            // Hold it here, kill all plugins when `tokio::run` is finished
            let plugins = launch_plugin(context.config_mut(), PluginMode::Server).expect("Failed to launch plugins");
            let mon = ::monitor::monitor_signal(plugins);

            let tcp_fut = run_tcp(SharedContext::new(context));

            vf.push(boxed_future(mon));
            vf.push(boxed_future(tcp_fut));
        }

        futures_unordered(vf).into_future().then(|res| -> io::Result<()> {
            match res {
                Ok(..) => Ok(()),
                Err((err, ..)) => Err(err),
            }
        })
    })
}
