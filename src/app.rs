use crate::config;
use crate::process::Process;

#[derive(Clone)]
pub struct App {
    name: String,
    port: u16,
    directory: String,
    command_config: config::CommandConfig,
    headers: hyper::HeaderMap,
    pub process: Process,
}

impl App {
    pub fn from_config(app_config: &config::App, auto_port: u16) -> Self {
        let process = Process::from_config(app_config, auto_port);

        Self {
            name: app_config.name.clone(),
            port: app_config.port.unwrap_or(auto_port),
            command_config: app_config.command_config.clone(),
            directory: app_config.full_path(),
            headers: app_config.parsed_headers(),
            process,
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

    pub fn start(&self) -> Result<(), String> {
        self.process.start()
    }

    pub fn is_running(&self) -> bool {
        self.process.is_running()
    }

    pub fn headers(&self) -> &hyper::HeaderMap {
        &self.headers
    }
}
