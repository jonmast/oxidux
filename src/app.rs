use futures::future::join_all;
use futures::Stream;

use crate::config;
use crate::process::Process;

#[derive(Clone, Debug)]
pub struct App {
    name: String,
    port: u16,
    directory: String,
    command_config: config::CommandConfig,
    headers: hyper::HeaderMap,
    pub processes: Vec<Process>,
    tld: String,
}

impl App {
    pub fn from_config(app_config: &config::App, auto_port: u16, tld: String) -> Self {
        let port = app_config.port.unwrap_or(auto_port);

        let processes = app_config
            .commands()
            .into_iter()
            .map(|(name, command)| Process::from_config(app_config, name, command, port))
            .collect();

        Self {
            name: app_config.name.clone(),
            port,
            command_config: app_config.command_config.clone(),
            directory: app_config.full_path(),
            headers: app_config.parsed_headers(),
            processes,
            tld,
        }
    }

    pub fn directory(&self) -> &str {
        self.directory.as_ref()
    }

    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn start(&self) {
        for process in &self.processes {
            if let Err(error) = process.start().await {
                eprint!(
                    "Process {} failed to start with error: {}",
                    process.name().await,
                    error
                )
            }
        }
    }

    pub async fn stop(&self) {
        for process in &self.processes {
            process.stop().await
        }
    }

    pub async fn is_running(&self) -> bool {
        for process in &self.processes {
            if process.is_running().await {
                return true;
            }
        }

        false
    }

    pub fn headers(&self) -> &hyper::HeaderMap {
        &self.headers
    }

    pub fn default_process(&self) -> Option<&Process> {
        self.processes.first()
    }

    pub async fn find_process(&self, name: &str) -> Option<&Process> {
        for process in &self.processes {
            if process.process_name().await == name {
                return Some(process);
            }
        }

        None
    }

    pub fn tld(&self) -> &str {
        &self.tld
    }

    pub async fn output_stream(&self) -> impl Stream<Item = (Process, String)> {
        eprintln!("Registering output listener");

        let streams = join_all(self.processes.iter().map(Process::register_output_watcher)).await;

        futures::stream::select_all(streams)
    }
}
