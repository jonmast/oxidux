use std::sync::Arc;
use std::time::Instant;

use futures::future::join_all;
use futures::Stream;
use tokio::sync::RwLock;

use crate::config;
use crate::process::Process;

// Follow Heroku convention of "web" as the label for primary process
const DEFAULT_PROCESS: &str = "web";

#[derive(Clone, Debug)]
pub struct App {
    /// App name. This serves as the primary app domain as well as a unique identifer.
    name: String,
    /// Port app server process binds to
    port: u16,
    /// App working directory
    directory: String,
    command_config: config::CommandConfig,
    /// Headers added to proxied request to the app
    headers: hyper::HeaderMap,
    /// List of processes for this app
    pub processes: Vec<Process>,
    /// Domain TLD suffix, defaults to ".test"
    tld: String,
    /// Alternate domain names for app
    aliases: Vec<String>,
    /// Last time app was accessed
    last_hit: Arc<RwLock<Instant>>,
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
            aliases: app_config.aliases.clone(),
            last_hit: Arc::new(RwLock::new(Instant::now())),
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

    pub async fn default_process(&self) -> Option<&Process> {
        self.find_process(DEFAULT_PROCESS)
            .await
            .or_else(|| self.processes.first())
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

    pub fn domains(&self) -> impl Iterator<Item = &String> + '_ {
        std::iter::once(&self.name).chain(self.aliases.iter())
    }

    /// Last time app was accessed
    pub(crate) async fn last_hit(&self) -> Instant {
        *self.last_hit.read().await
    }

    /// Refresh the last hit time
    pub(crate) async fn touch(&self) {
        *self.last_hit.write().await = Instant::now();
    }
}
