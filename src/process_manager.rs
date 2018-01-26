use hyper::header::Host;

use process::Process;
use config::Config;

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

    pub fn find_process(&self, host: &Host) -> Option<&Process> {
        let hostname = host.hostname();

        let app_name = match hostname.find(".") {
            Some(tld_start) => &hostname[0..tld_start],
            None => hostname,
        };

        println!("Looking for app {}", app_name);
        self.processes
            .iter()
            .find(|&&ref process| process.app_name() == app_name)
    }

    pub fn start_processes(&self) {
        for process in &self.processes {
            process.start()
        }
    }
}
