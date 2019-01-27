extern crate ansi_term;
extern crate dirs;
extern crate futures;
extern crate hyper;
extern crate nix;
extern crate serde_json;
extern crate shellexpand;
extern crate tokio;
extern crate tokio_codec;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_process;
extern crate tokio_uds;
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
use crate::process_manager::ProcessManager;
pub mod config;
use crate::config::Config;
pub mod client;
pub mod ipc_command;
mod ipc_listener;
mod ipc_response;
mod output;

pub fn run_server(config: Config) {
    hyper::rt::run(future::lazy(move || {
        let process_manager = start_process_manager(&config);

        ipc_listener::start_ipc_sock(process_manager.clone());

        let client = Client::new();

        let proxy = move || {
            let client = client.clone();
            let process_manager = process_manager.clone();

            service_fn(move |req| proxy::handle_request(&req, &client, &process_manager))
        };

        Server::bind(&build_address(&config))
            .serve(proxy)
            .map_err(|err| eprintln!("serve error: {:?}", err))
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

        assert_eq!(addr.port(), 80);
        let localhost: std::net::IpAddr = "127.0.0.1".parse().unwrap();
        assert_eq!(addr.ip(), localhost);
    }
}
