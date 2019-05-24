use crate::config::Config;
use crate::process::Process;

#[derive(Clone)]
pub struct ProcessManager {
    pub processes: Vec<Process>,
}

const PORT_START: u16 = 7500;

impl ProcessManager {
    pub fn new(config: &Config) -> ProcessManager {
        let processes = config
            .apps
            .iter()
            .enumerate()
            .map(|(idx, process_config)| {
                Process::from_config(&process_config, PORT_START + (idx as u16))
            })
            .collect();

        ProcessManager { processes }
    }

    pub fn find_process(&self, hostname: &str) -> Option<&Process> {
        let parts: Vec<&str> = hostname.split('.').collect();
        let &app_name = parts.get(parts.len() - 2).unwrap_or(&hostname);

        eprintln!("Looking for app {}", app_name);
        self.processes
            .iter()
            .find(|process| process.app_name() == app_name)
    }

    pub fn find_process_for_directory(&self, directory: &str) -> Option<&Process> {
        self.processes
            .iter()
            .find(|process| directory.starts_with(&process.directory()))
    }

    pub fn shutdown(&self) {
        for process in &self.processes {
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

    #[test]
    fn find_process_with_subdomain() {
        let app_config = config::App {
            name: "the_app".into(),
            directory: "".into(),
            port: None,
            command: "".into(),
            headers: None,
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
