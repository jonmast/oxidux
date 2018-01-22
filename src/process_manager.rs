use hyper::header::Host;

use process::Process;
use config;

pub struct ProcessManager {
    processes: Vec<Process>,
}

impl ProcessManager {
    pub fn new(config: Config) -> ProcessManager {
        let config = config::read_config();
        println!("Parsed config {:?}", config);
        let test_process = Process::new("foo".to_string(), 80);

        ProcessManager {
            processes: vec![test_process],
        }
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
            .find(|&&ref process| process.app_name == app_name)
    }
}
