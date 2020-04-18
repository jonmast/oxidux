use futures::future::FutureExt;
use hyper::body::Buf;
use hyper::{Body, Client, Request};
use oxidux;
use oxidux::config::{App, CommandConfig, Config, ProxyConfig};
use oxidux::process_manager::ProcessManager;
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::delay_for;

#[tokio::test]
async fn it_proxies_to_configured_port() {
    let (tx, rx) = oneshot::channel::<()>();
    let port = 9585;
    let app_name = "proxy_test";
    let tld = "test";
    let app = App {
        name: app_name.into(),
        directory: "/".into(),
        port: Some(port),
        headers: Default::default(),
        command_config: CommandConfig::Command("/bin/true".into()),
    };
    let proxy_port = 9584;
    let config = Config {
        general: ProxyConfig {
            proxy_port,
            domain: tld.into(),
            ..Default::default()
        },
        apps: vec![app],
    };

    ProcessManager::initialize(&config);

    tokio::spawn(async {
        oxidux::proxy::start_server(config, rx.map(|_| ())).await;
    });

    let _server = HelperCommand::run_echo_server(port).unwrap();

    // Add an arbitrary delay to give the server time to boot
    delay_for(Duration::from_millis(50)).await;

    // Send request to proxy
    let client = Client::new();
    let greeting = "Hello!";
    let app_host = format!("{}.{}", app_name, tld);
    let uri: hyper::http::Uri = format!("http://localhost:{}/proxy-test", proxy_port)
        .parse()
        .unwrap();
    let request = Request::builder()
        .uri(uri)
        .header("host", app_host.clone())
        .body(Body::from(greeting))
        .unwrap();

    let response = client.request(request).await.unwrap();
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
    fn run_echo_server(port: u16) -> Result<Self, failure::Error> {
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
