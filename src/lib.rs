#![warn(clippy::all)]

use tokio::runtime::Runtime;
use tokio::signal;
use tokio::sync::oneshot;

mod proxy;

mod app;
mod process;
mod process_manager;
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

async fn ctrlc_listener(process_manager: ProcessManager) {
    let (tx, rx) = oneshot::channel::<()>();

    let mut shutdown_tx = Some(tx);

    let signal_handler = async move {
        loop {
            signal::ctrl_c().await.unwrap();

            if let Some(tx) = shutdown_tx.take() {
                eprintln!("Gracefully shutting down");

                process_manager.shutdown();

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
        let process_manager = ProcessManager::new(&config);

        let shutdown_rx = ctrlc_listener(process_manager.clone());

        ipc_listener::start_ipc_sock(process_manager.clone());

        println!("Spinning up server");
        proxy::start_server(config, process_manager, shutdown_rx).await
    });
}
