use crate::app::App;
use crate::config::Config;

#[derive(Clone)]
pub struct ProcessManager {
    pub apps: Vec<App>,
}

const PORT_START: u16 = 7500;

impl ProcessManager {
    pub fn new(config: &Config) -> ProcessManager {
        let apps = config
            .apps
            .iter()
            .enumerate()
            .map(|(idx, process_config)| {
                App::from_config(&process_config, PORT_START + (idx as u16))
            })
            .collect();

        ProcessManager { apps }
    }

    /// Find the app associated with a given hostname
    pub fn find_app(&self, hostname: &str) -> Option<&App> {
        let parts = hostname.split('.');
        // Penultimate segment should contain app name
        let app_name = parts.rev().nth(1).unwrap_or(hostname);

        eprintln!("Looking for app {}", app_name);
        self.apps.iter().find(|app| app.name() == app_name)
    }

    pub fn find_app_for_directory(&self, directory: &str) -> Option<&App> {
        self.apps
            .iter()
            .find(|app| directory.starts_with(&app.directory()))
    }

    pub fn shutdown(&self) {
        for process in self.apps.iter().map(|app| &app.process) {
            if process.is_running() {
                process.stop();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use std::collections::HashMap;

    #[test]
    fn find_process_with_subdomain() {
        let app_config = config::App {
            name: "the_app".into(),
            directory: "".into(),
            port: None,
            command: "".into(),
            headers: HashMap::default(),
        };

        let process = Process::from_config(&app_config, 0);
        let process_manager = ProcessManager {
            processes: vec![process],
        };

        let found_app = process_manager
            .find_process("subdomain.the_app.test")
            .expect("Failed to find process by hostname");

        assert!(found_app.app_name() == "the_app")
    }
}
