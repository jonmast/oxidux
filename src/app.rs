use crate::config;
use crate::process::Process;

#[derive(Clone)]
pub struct App {
    name: String,
    port: u16,
    directory: String,
    command_config: config::CommandConfig,
    headers: hyper::HeaderMap,
    processes: Vec<Process>,
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

    pub fn start(&self) {
        for process in &self.processes {
            process.start().unwrap_or_else(|error| {
                eprint!(
                    "Process {} failed to start with error: {}",
                    process.name(),
                    error
                )
            })
        }
    }

    pub fn stop(&self) {
        for process in &self.processes {
            process.stop()
        }
    }

    pub fn is_running(&self) -> bool {
        self.processes.iter().any(|process| process.is_running())
    }

    pub fn headers(&self) -> &hyper::HeaderMap {
        &self.headers
    }

    pub fn default_process(&self) -> Option<&Process> {
        self.processes.first()
    }
    pub fn find_process(&self, name: &str) -> Option<&Process> {
        self.processes
            .iter()
            .find(|process| process.process_name() == name)
    }

    pub fn tld(&self) -> &str {
        &self.tld
    }
}
