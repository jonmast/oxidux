use std::net::SocketAddr;

use futures::future::{self, Future};

use futures::sync::oneshot;
use hyper::service::service_fn;
use hyper::{Client, Server};
use tokio::prelude::*;
use tokio_signal;

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

fn ctrlc_listener(process_manager: ProcessManager) -> impl Future<Item = (), Error = ()> {
    let (tx, rx) = oneshot::channel::<()>();

    let mut shutdown_tx = Some(tx);

    let signal_handler = tokio_signal::ctrl_c().flatten_stream().for_each(move |()| {
        if let Some(tx) = shutdown_tx.take() {
            eprintln!("Gracefully shutting down");

            process_manager.shutdown();
            for process in &process_manager.processes {
                if process.is_running() {
                    process.stop();
                }
            }

            tx.send(()).unwrap();
        } else {
            eprintln!("Forcibly shutting down");
            std::process::exit(1);
        }

        Ok(())
    });
    tokio::spawn(signal_handler.map_err(|err| eprintln!("Error in signal handler {}", err)));

    rx.map_err(|_| eprintln!("Error in shutdown channel"))
        .and_then(|_| Ok(()))
}

pub fn run_server(config: Config) {
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(future::lazy(move || {
            let process_manager = ProcessManager::new(&config);

            let shutdown_rx = ctrlc_listener(process_manager.clone());

            ipc_listener::start_ipc_sock(process_manager.clone());

            let client = Client::new();

            let proxy = move || {
                let client = client.clone();
                let process_manager = process_manager.clone();

                service_fn(move |req| proxy::handle_request(&req, &client, &process_manager))
            };

            let addr = &build_address(&config);
            println!("Starting proxy server on {}", addr);

            Server::bind(addr)
                .serve(proxy)
                .with_graceful_shutdown(shutdown_rx.and_then(|_| Ok(())))
                .map_err(|err| eprintln!("serve error: {:?}", err))
        }))
        .unwrap();
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
