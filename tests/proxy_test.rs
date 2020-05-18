use futures::future::FutureExt;
use hyper::body::Buf;
use hyper::{Body, Client, Request};
use oxidux::config::{Config, ProxyConfig};
use oxidux::process_manager::ProcessManager;
use std::env;
use std::fs::{create_dir, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::delay_for;

#[path = "./helpers/test_utils.rs"]
mod test_utils;

#[tokio::test]
async fn it_proxies_to_configured_port() {
    let app_name = "proxy_test";
    let port = 9585;
    let config_dir = test_utils::temp_dir();
    let app_dir = config_dir.join("apps");
    create_dir(&app_dir).unwrap();

    let mut app_file = File::create(&app_dir.join("proxy_test.toml")).unwrap();

    app_file
        .write_all(
            format!(
                "
name = '{}'
directory = '/'
port = {}
command = 'sleep 10; {}'
",
                app_name,
                port,
                test_process_path("echo-server").unwrap().to_str().unwrap()
            )
            .as_ref(),
        )
        .unwrap();

    let (tx, rx) = oneshot::channel::<()>();
    let tld = "test";
    let proxy_port = 9584;
    let config = Config {
        general: ProxyConfig {
            proxy_port,
            config_dir: config_dir.to_path_buf(),
            domain: tld.into(),
            ..Default::default()
        },
    };

    ProcessManager::initialize(&config);

    tokio::spawn(async {
        tokio::time::timeout(
            Duration::from_secs(5),
            oxidux::proxy::start_server(config, rx.map(|_| ())),
        )
        .await
        .unwrap();
    });

    // TODO: remove this and let the app autostart
    let _server = HelperCommand::run_echo_server(port).unwrap();

    // Send request to proxy
    let client = Client::new();
    let greeting = "Hello!";
    let app_host = format!("{}.{}", app_name, tld);
    let uri: hyper::http::Uri = format!("http://localhost:{}/proxy-test", proxy_port)
        .parse()
        .unwrap();
    let request = Request::builder()
        .uri(&uri)
        .header("host", app_host.clone())
        .body(Body::from(greeting))
        .unwrap();

    // Add an arbitrary delay to give the server time to boot
    delay_for(Duration::from_millis(100)).await;

    let response = tokio::time::timeout(Duration::from_secs(1), client.request(request))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(response.status(), 200);

    let mut buffer = hyper::body::aggregate(response.into_body()).await.unwrap();
    let data: serde_json::Value =
        serde_json::from_str(std::str::from_utf8(&buffer.to_bytes()).unwrap()).unwrap();

    assert_eq!(data["url"], "/proxy-test");
    assert_eq!(data["headers"]["host"], app_host);
    assert_eq!(data["body"], greeting);

    tx.send(()).unwrap();
}

struct HelperCommand {
    process: std::process::Child,
}

impl HelperCommand {
    fn run_echo_server(port: u16) -> color_eyre::Result<Self> {
        let helper_exe = test_process_path("echo-server");
        use std::process::Command;

        let child = Command::new(&helper_exe.unwrap())
            .env("PORT", port.to_string())
            .spawn()?;

        Ok(Self { process: child })
    }
}

impl Drop for HelperCommand {
    fn drop(&mut self) {
        self.process.kill().unwrap();
    }
}

fn test_process_path(name: &str) -> Option<PathBuf> {
    env::current_exe().ok().and_then(|p| {
        p.parent().map(|p| {
            p.with_file_name(name)
                .with_extension(env::consts::EXE_EXTENSION)
        })
    })
}
