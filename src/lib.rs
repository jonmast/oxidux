#![warn(clippy::all)]
#![feature(async_await)]

use tokio::sync::oneshot;

use tokio::prelude::*;
use tokio::runtime::Runtime;
use tokio_net::signal;

mod proxy;

mod app;
mod process;
mod process_manager;
use crate::process_manager::ProcessManager;
pub mod config;
use crate::config::Config;
pub mod client;
// #[cfg(target_os = "macos")]
// mod dns;
pub mod ipc_command;
mod ipc_listener;
mod ipc_response;
mod output;
mod procfile;

async fn ctrlc_listener(process_manager: ProcessManager) {
    let (tx, rx) = oneshot::channel::<()>();

    let mut shutdown_tx = Some(tx);

    let ctrl_c = signal::CtrlC::new().unwrap();
    let signal_handler = ctrl_c.for_each(move |()| {
        if let Some(tx) = shutdown_tx.take() {
            eprintln!("Gracefully shutting down");

            process_manager.shutdown();

            tx.send(()).unwrap();
        } else {
            eprintln!("Forcibly shutting down");
            std::process::exit(1);
        }

        tokio::future::ready(())
    });
    tokio::spawn(signal_handler);

    rx.await.ok();
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

    let runtime = Runtime::new().unwrap();
    runtime.block_on(async {
        let process_manager = ProcessManager::new(&config);

        let shutdown_rx = ctrlc_listener(process_manager.clone());

        ipc_listener::start_ipc_sock(process_manager.clone());

        // #[cfg(target_os = "macos")]
        // dns::start_dns_server(config.general.dns_port, &config.general.domain)
        //     .expect("Failed to start DNS server");

        println!("Spinning up server");
        proxy::start_server(config, process_manager, shutdown_rx).await
    });

    runtime.shutdown_now();
}
