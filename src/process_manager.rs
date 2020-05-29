use crate::app::App;
use crate::config::Config;
use once_cell::sync::OnceCell;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::delay_for;

#[derive(Debug)]
pub struct ProcessManager {
    pub apps: Vec<App>,
    config: Config,
}

const PORT_START: u16 = 7500;
static INSTANCE: OnceCell<RwLock<ProcessManager>> = OnceCell::new();

impl ProcessManager {
    pub fn initialize(config: &Config) {
        INSTANCE.set(RwLock::new(Self::new(config))).unwrap();
    }

    fn new(config: &Config) -> ProcessManager {
        let apps = Vec::new();

        let config = config.clone();
        ProcessManager { apps, config }
    }

    pub(crate) fn global() -> &'static RwLock<ProcessManager> {
        INSTANCE
            .get()
            .expect("Attempted to use ProcessManager before it was initalized")
    }

    pub(crate) fn find_app_by_name(&self, app_name: &str) -> Option<&App> {
        eprintln!("Looking for app {}", app_name);

        self.apps.iter().find(|app| app.name() == app_name)
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn add_app(&mut self, new_app: crate::config::App) -> App {
        let highest_port = self.apps.iter().map(App::port).max().unwrap_or(PORT_START);

        let app = App::from_config(
            &new_app,
            highest_port + 1,
            self.config.general.domain.clone(),
        );

        self.apps.push(app.clone());

        app
    }

    pub fn find_app_for_directory(&self, directory: &str) -> Option<&App> {
        self.apps
            .iter()
            .find(|app| directory.starts_with(&app.directory()))
    }

    /// Stop all apps
    pub async fn shutdown(&mut self) {
        for app in self.apps.iter() {
            if app.is_running().await {
                app.stop().await;
            }
        }

        // Poll processes until they stop
        'outer: loop {
            delay_for(Duration::from_millis(200)).await;

            for app in &self.apps {
                if app.is_running().await {
                    continue 'outer;
                }
            }

            // Drop processes to clean up any leftover watchers
            self.apps.clear();

            return;
        }
    }

    pub(crate) fn remove_app_by_name(&mut self, app_name: &str) {
        self.apps.retain(|a| a.name() != app_name);
    }
}
