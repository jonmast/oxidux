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
        let app_name = match hostname.find('.') {
            Some(tld_start) => &hostname[0..tld_start],
            None => hostname,
        };

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
