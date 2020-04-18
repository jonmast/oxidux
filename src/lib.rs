#![warn(clippy::all)]

use std::time::Duration;

use tokio::runtime::Runtime;
use tokio::signal;
use tokio::sync::oneshot;

pub mod proxy;

mod app;
mod process;
pub mod process_manager;
use crate::process_manager::ProcessManager;
pub mod config;
use crate::config::Config;
pub mod client;
#[cfg(target_os = "macos")]
mod dns;
pub mod ipc_command;
mod ipc_listener;
mod ipc_response;
mod output;
mod procfile;

async fn ctrlc_listener() {
    let (tx, rx) = oneshot::channel::<()>();

    let mut shutdown_tx = Some(tx);

    let signal_handler = async move {
        loop {
            signal::ctrl_c().await.unwrap();

            if let Some(tx) = shutdown_tx.take() {
                eprintln!("Gracefully shutting down");

                ProcessManager::global().write().await.shutdown().await;

                tx.send(()).unwrap();
            } else {
                eprintln!("Forcibly shutting down");
                std::process::exit(1);
            }
        }
    };

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

    let mut runtime = Runtime::new().unwrap();

    #[cfg(target_os = "macos")]
    runtime.enter(|| {
        dns::start_dns_server(config.general.dns_port, &config.general.domain, &runtime)
            .expect("Failed to start DNS server");
    });

    runtime.block_on(async {
        ProcessManager::initialize(&config);

        let shutdown_rx = ctrlc_listener();

        ipc_listener::start_ipc_sock();

        proxy::start_server(config, shutdown_rx).await;
    });

    runtime.shutdown_timeout(Duration::from_millis(500));
}
