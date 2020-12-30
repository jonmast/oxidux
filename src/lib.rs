#![warn(clippy::all)]

use std::time::Duration;

use tokio::runtime::Runtime;

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
mod host_resolver;
pub mod ipc_command;
mod ipc_listener;
mod ipc_response;
mod output;
mod procfile;
mod signals;
mod tmux;

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
        ProcessManager::initialize(&config);

        tokio::spawn(ProcessManager::monitor_idle_timeout());

        #[cfg(target_os = "macos")]
        dns::start_dns_server(config.general.dns_port, &config.general.domain)
            .expect("Failed to start DNS server");

        let shutdown_rx = signals::ctrlc_listener();

        ipc_listener::start_ipc_sock();

        proxy::start_server(config, shutdown_rx).await;
    });

    runtime.shutdown_timeout(Duration::from_millis(500));
}

// File for shared helpers between integration and unit tests
#[cfg(test)]
#[path = "../tests/helpers/test_utils.rs"]
mod test_utils;
