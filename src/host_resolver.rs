use crate::app::App;
use crate::process_manager::ProcessManager;

/// Find the associated `App` for a given hostname
///
/// Looks in running apps and then falls back to creating the app from config
pub(crate) async fn resolve(host: &str) -> Option<App> {
    let process_manager = ProcessManager::global().read().await;
    let existing_app = { process_manager.find_app(host).cloned() };

    if existing_app.is_some() {
        return existing_app;
    }

    let app_config = process_manager
        .config()
        .find_app_by_host(host)
        .await?
        .clone();
    drop(process_manager);

    // Upgrade to a write lock
    let mut process_manager = ProcessManager::global().write().await;
    Some(process_manager.add_app(app_config))
}
