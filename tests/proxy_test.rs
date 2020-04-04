use futures::future::FutureExt;
use futures::stream::StreamExt;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Request, Response, Server};
use oxidux;
use oxidux::config::{App, CommandConfig, Config, ProxyConfig};
use oxidux::process_manager::ProcessManager;
use std::{convert::Infallible, net::SocketAddr};
use tokio::sync::oneshot;

#[tokio::test]
async fn it_proxies_to_configured_port() {
    let (tx, rx) = oneshot::channel::<()>();
    let port = 9585;
    let app_name = "proxy_test";
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
            ..Default::default()
        },
        apps: vec![app],
    };

    ProcessManager::initialize(&config);

    tokio::spawn(async {
        oxidux::proxy::start_server(config, rx.map(|_| ())).await;
    });

    // Test server that we're proxying to
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let make_svc = make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(handle)) });

    let server = Server::bind(&addr).serve(make_svc);

    tokio::spawn(async {
        if let Err(e) = server.await {
            eprintln!("Error spawning test server: {}", e);
        }
    });

    // Send request to proxy
    let client = Client::new();
    let uri: hyper::http::Uri = format!("http://localhost:{}", proxy_port).parse().unwrap();
    let request = Request::builder()
        .uri(uri)
        .header("host", app_name)
        .body(Body::empty())
        .unwrap();

    let response = client.request(request).await.unwrap();
    let body = response
        .into_body()
        .fold(vec![], |mut acc, chunk| async {
            acc.extend_from_slice(&chunk.unwrap());
            acc
        })
        .await;

    let str_body = String::from_utf8(body).unwrap();

    assert_eq!(str_body, "Hello, World!");

    tx.send(()).unwrap();
}

async fn handle(_: Request<Body>) -> Result<Response<Body>, Infallible> {
    Ok(Response::new("Hello, World!".into()))
}
