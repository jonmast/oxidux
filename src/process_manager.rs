use crate::app::App;
use crate::config::Config;
use once_cell::sync::OnceCell;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct ProcessManager {
    pub apps: Vec<App>,
}

const PORT_START: u16 = 7500;
static INSTANCE: OnceCell<RwLock<ProcessManager>> = OnceCell::new();

impl ProcessManager {
    pub fn initialize(config: &Config) {
        INSTANCE.set(RwLock::new(Self::new(config))).unwrap();
    }

    fn new(config: &Config) -> ProcessManager {
        let apps = config
            .apps
            .iter()
            .enumerate()
            .map(|(idx, process_config)| {
                App::from_config(
                    &process_config,
                    PORT_START + (idx as u16),
                    config.general.domain.clone(),
                )
            })
            .collect();

        ProcessManager { apps }
    }

    pub(crate) fn global() -> &'static RwLock<ProcessManager> {
        INSTANCE
            .get()
            .expect("Attempted to use ProcessManager before it was initalized")
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

    /// Stop all apps
    pub async fn shutdown(&self) {
        for app in self.apps.iter() {
            if app.is_running().await {
                app.stop().await;
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
    fn find_app_with_subdomain() {
        let app_config = config::App {
            name: "the_app".into(),
            directory: "".into(),
            port: None,
            command_config: config::CommandConfig::Command("".to_string()),
            headers: HashMap::default(),
        };

        let app = App::from_config(&app_config, 0, "test".to_string());
        let process_manager = ProcessManager { apps: vec![app] };

        let found_app = process_manager
            .find_app("subdomain.the_app.test")
            .expect("Failed to find app by hostname");

        assert!(found_app.name() == "the_app")
    }
}
