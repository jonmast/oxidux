extern crate futures;
extern crate hyper;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_process;
extern crate toml;
extern crate url;

#[macro_use]
extern crate serde_derive;

use std::net::SocketAddr;

use futures::future::Future;
use futures::Stream;

use tokio_core::reactor::{Core, Handle};

use hyper::server::Http;

mod proxy;
use proxy::Proxy;

mod process;
mod process_manager;
use process_manager::ProcessManager;
pub mod config;
use config::Config;
mod output;

pub fn run_server(config: Config) {
    let mut core = Core::new().unwrap();
    let client_handle = core.handle();

    let process_manager = start_process_manager(&config, &core.handle());

    let server = Http::new()
        .serve_addr_handle(&build_address(&config), &core.handle(), move || {
            Ok(Proxy::new(client_handle.clone(), process_manager.clone()))
        })
        .unwrap();

    let server_handle = core.handle();
    let spawn_handle = server_handle.clone();

    server_handle.spawn(
        server
            .for_each(move |conn| {
                spawn_handle.spawn(
                    conn.map(|_| ())
                        .map_err(|err| println!("serve error: {:?}", err)),
                );
                Ok(())
            })
            .map_err(|_| ()),
    );

    core.run(futures::future::empty::<(), ()>()).unwrap();
}

fn start_process_manager(config: &Config, handle: &Handle) -> ProcessManager {
    let process_manager = ProcessManager::new(&config);
    process_manager.start_processes(handle);

    process_manager
}

fn build_address(config: &Config) -> SocketAddr {
    let port = config.general.proxy_port;
    format!("127.0.0.1:{}", port).parse().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_bind_address_from_config() {
        let config = config::Config {
            apps: Vec::new(),
            general: config::ProxyConfig { proxy_port: 80 },
        };

        let addr = build_address(&config);

        assert_eq!(addr.pirt(), 80);
        assert_eq!(addr.ip(), "127.0.0.1");
    }
}
