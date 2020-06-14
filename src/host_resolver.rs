use crate::app::App;
use crate::process_manager::ProcessManager;

/// Find the associated `App` for a given hostname
///
/// Looks in running apps and then falls back to creating the app from config
pub(crate) async fn resolve(host: &str) -> Option<App> {
    let process_manager = ProcessManager::global().read().await;

    for app in &process_manager.apps {
        let domains = app.domains();

        if matches_domains(host, domains) {
            return Some(app.clone());
        }
    }

    let mut app_config = None;

    for config in process_manager.config().app_configs().await {
        let domains = config.domains();

        if matches_domains(host, domains) {
            app_config = Some(config.clone());
            break;
        }
    }
    drop(process_manager);
    let app_config = app_config?;

    // Upgrade to a write lock
    let mut process_manager = ProcessManager::global().write().await;
    Some(process_manager.add_app(app_config))
}

/// Check if the provided hostname matches one of the app domains
fn matches_domains<'a>(host: &str, mut domains: impl Iterator<Item = &'a String>) -> bool {
    let parts = host.split('.');
    // Penultimate segment should contain "domain", we don't currently allow specifying subdomains
    let needle = parts.rev().nth(1).unwrap_or(host);
    domains.any(|domain| domain == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{App, Config};

    #[tokio::test]
    async fn resolve_test() {
        ProcessManager::initialize(&Config::default());
        let app_config = App {
            name: "appname".to_string(),
            aliases: vec!["appalias".to_string()],
            ..Default::default()
        };
        ProcessManager::global().write().await.add_app(app_config);

        let app = resolve("appname.test").await.unwrap();
        assert_eq!("appname", app.name());

        let aliased_app = resolve("appalias.test").await.unwrap();
        assert_eq!("appname", aliased_app.name());

        let subdomain_app = resolve("subdomain.appalias.test").await.unwrap();
        assert_eq!("appname", subdomain_app.name());
    }
}
