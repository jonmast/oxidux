#![warn(clippy::all)]

use futures::future::{self, Future};
use futures::sync::oneshot;

use tokio::prelude::*;
use tokio::runtime::Runtime;
use tokio_signal;

mod proxy;

mod app;
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
mod procfile;

fn ctrlc_listener(process_manager: ProcessManager) -> impl Future<Item = (), Error = ()> {
    let (tx, rx) = oneshot::channel::<()>();

    let mut shutdown_tx = Some(tx);

    let signal_handler = tokio_signal::ctrl_c().flatten_stream().for_each(move |()| {
        if let Some(tx) = shutdown_tx.take() {
            eprintln!("Gracefully shutting down");

            process_manager.shutdown();

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

fn server_running() -> bool {
    if let Ok(response) = ipc_command::ping_server() {
        response == "pong"
    } else {
        false
    }
}

pub fn run_server(config: Config) {
    if server_running() {
        return eprint!("Error: server is already running");
    }

    let mut runtime = Runtime::new().unwrap();
    runtime
        .block_on(future::lazy(move || {
            let process_manager = ProcessManager::new(&config);

            let shutdown_rx = ctrlc_listener(process_manager.clone());

            ipc_listener::start_ipc_sock(process_manager.clone());

            proxy::start_server(&config, process_manager, shutdown_rx.and_then(|_| Ok(())))
        }))
        .unwrap();

    runtime.shutdown_now();
}
