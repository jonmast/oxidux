extern crate futures;
extern crate hyper;
extern crate tokio;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_process;
extern crate toml;
extern crate url;

#[macro_use]
extern crate serde_derive;

use std::net::SocketAddr;

use futures::future::{self, Future};

use hyper::service::service_fn;
use hyper::{Client, Server};

mod proxy;

mod process;
mod process_manager;
use process_manager::ProcessManager;
pub mod config;
use config::Config;
mod output;

pub fn run_server(config: Config) {
    hyper::rt::run(future::lazy(move || {
        let process_manager = start_process_manager(&config);

        let client = Client::new();

        let proxy = move || {
            let client = client.clone();
            let process_manager = process_manager.clone();

            service_fn(move |req| proxy::handle_request(req, &client, &process_manager))
        };

        Server::bind(&build_address(&config))
            .serve(proxy)
            .map_err(|err| println!("serve error: {:?}", err))
    }));
}

fn start_process_manager(config: &Config) -> ProcessManager {
    let process_manager = ProcessManager::new(&config);
    process_manager.start_processes();

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
