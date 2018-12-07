use crate::config::Config;
use crate::process::Process;

#[derive(Clone)]
pub struct ProcessManager {
    processes: Vec<Process>,
}

impl ProcessManager {
    pub fn new(config: &Config) -> ProcessManager {
        let processes = config
            .apps
            .iter()
            .map(|process_config| Process::from_config(&process_config))
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
            .find(|ref process| process.app_name() == app_name)
    }

    pub fn find_process_for_directory(&self, directory: &str) -> Option<&Process> {
        self.processes
            .iter()
            .find(|ref process| directory.starts_with(&process.directory()))
    }

    pub fn start_processes(&self) {
        for process in &self.processes {
            process.start()
        }
    }
}
