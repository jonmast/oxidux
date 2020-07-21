use std::future::Future;
use std::net::{SocketAddr, TcpListener};

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Request, Response, Server, Uri};
use url::Url;

use crate::host_resolver;

mod autostart_response;
mod host_missing;
mod meta_server;
mod tls;

use crate::{app::App, config::Config, process_manager::ProcessManager};

const ERROR_MESSAGE: &str = "No response from server";

async fn error_response(error: &hyper::Error, app: &App) -> Response<Body> {
    eprintln!("Request to backend failed with error \"{}\"", error);

    if app.is_running().await {
        let body = Body::from(ERROR_MESSAGE);
        Response::builder()
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(body)
            .unwrap()
    } else {
        app.start().await;

        autostart_response::autostart_response()
    }
}

pub async fn start_server(config: Config, shutdown_handler: impl Future<Output = ()>) {
    let [http_socket, https_socket] = get_proxy_sockets();
    let (addr, server): (_, color_eyre::Result<_>) = if let Ok(listener) = http_socket {
        let addr = listener.local_addr().unwrap();
        (addr, Server::from_tcp(listener).map_err(|e| e.into()))
    } else {
        use eyre::WrapErr;
        let addr = &build_address(&config);
        (
            *addr,
            Server::try_bind(addr).context("Failed to start proxy on specified port"),
        )
    };

    if let Ok(listener) = https_socket {
        tokio::spawn(async move { tls::tls_server(&config.clone(), listener).await.unwrap() });
    }

    eprintln!("Starting proxy server on {}", addr);

    let proxy =
        make_service_fn(|_| async move { Ok::<_, eyre::Error>(service_fn(handle_request)) });

    let server = server
        .unwrap()
        .serve(proxy)
        .with_graceful_shutdown(shutdown_handler);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}

#[cfg(not(target_os = "macos"))]
fn get_proxy_sockets() -> [color_eyre::Result<TcpListener>; 2] {
    let mut listenfd = listenfd::ListenFd::from_env();

    [
        listenfd
            .take_tcp_listener(0)
            .map_err(Into::into)
            .and_then(|option| option.ok_or_else(|| eyre::eyre!("No socket provided"))),
        listenfd
            .take_tcp_listener(1)
            .map_err(Into::into)
            .and_then(|option| option.ok_or_else(|| eyre::eyre!("No socket provided"))),
    ]
}

#[cfg(target_os = "macos")]
pub(crate) mod launchd;
#[cfg(target_os = "macos")]
fn get_proxy_sockets() -> [color_eyre::Result<TcpListener>; 2] {
    [
        launchd::get_tcp_socket("HttpSocket").map_err(Into::into),
        launchd::get_tcp_socket("HttpsSocket").map_err(Into::into),
    ]
}

async fn handle_request(mut request: Request<Body>) -> color_eyre::Result<Response<Body>> {
    let host = request
        .headers()
        .get("HOST")
        .and_then(|v| v.to_str().ok())
        // HTTP2 requests only set uri, not host header
        .or_else(|| request.uri().host())
        .unwrap_or_default();

    eprintln!("Serving request for host {:?}", host);
    eprintln!("Full req URI {}", request.uri());

    let app = {
        match host_resolver::resolve(&host).await {
            Some(app) => app,
            None => {
                let process_manager = ProcessManager::global_read().await;
                return Ok(host_missing::missing_host_response(host, &process_manager).await);
            }
        }
    };

    if meta_server::is_meta_request(&request) {
        return meta_server::handle_request(request, app).await;
    }

    let destination_url = app_url(&app, request.uri());
    *request.uri_mut() = destination_url;
    *request.version_mut() = hyper::Version::HTTP_11;

    app.touch().await;

    // Apply header overrides from config
    request.headers_mut().extend(app.headers().clone());

    let client = Client::new();
    let result = client.request(request).await;

    match result {
        Ok(response) => {
            eprintln!("Proxying response");

            Ok(response)
        }
        Err(e) => Ok(error_response(&e, &app).await),
    }
}

fn build_address(config: &Config) -> SocketAddr {
    let port = config.general.proxy_port;
    format!("127.0.0.1:{}", port).parse().unwrap()
}

fn app_url(app: &App, request_url: &Uri) -> Uri {
    let base_url = Url::parse("http://localhost/").unwrap();

    let mut destination_url = base_url
        .join(request_url.path_and_query().unwrap().as_str())
        .expect("Invalid request URL");

    destination_url.set_port(Some(app.port())).unwrap();

    eprintln!("Starting request to backend {}", destination_url);

    destination_url.as_str().parse().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;

    #[test]
    fn build_bind_address_from_config() {
        let config = config::Config {
            general: config::ProxyConfig {
                proxy_port: 80,
                ..Default::default()
            },
        };

        let addr = build_address(&config);

        assert_eq!(addr.port(), 80);
        let localhost: std::net::IpAddr = "127.0.0.1".parse().unwrap();
        assert_eq!(addr.ip(), localhost);
    }

    #[test]
    fn app_url_test() {
        let config = config::App {
            name: "testapp".to_string(),
            command_config: config::CommandConfig::Procfile,
            port: Some(42),
            ..Default::default()
        };
        let app = App::from_config(&config, 0, "test".to_string());
        let source_uri = "http://testapp.test/path?query=true".parse().unwrap();

        let result = app_url(&app, &source_uri);

        assert_eq!(result, "http://localhost:42/path?query=true")
    }
}
