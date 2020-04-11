use std::future::Future;
use std::net::{SocketAddr, TcpListener};

use hyper::service::{make_service_fn, service_fn};
use hyper::{client::HttpConnector, Body, Client, Request, Response, Server, Uri};
use url::Url;

mod autostart_response;
mod host_missing;
mod meta_server;

use crate::{app::App, config::Config, process_manager::ProcessManager};

const ERROR_MESSAGE: &str = "No response from server";

fn error_response(error: &hyper::Error, app: &App) -> Response<Body> {
    eprintln!("Request to backend failed with error \"{}\"", error);

    if app.is_running() {
        let body = Body::from(ERROR_MESSAGE);
        Response::builder()
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(body)
            .unwrap()
    } else {
        app.start();

        autostart_response::autostart_response()
    }
}

pub async fn start_server(config: Config, shutdown_handler: impl Future<Output = ()>) {
    let (addr, server) = if let Ok(listener) = get_activation_socket() {
        let addr = listener.local_addr().unwrap();
        (addr, Server::from_tcp(listener).unwrap())
    } else {
        let addr = &build_address(&config);
        (*addr, Server::bind(addr))
    };

    eprintln!("Starting proxy server on {}", addr);

    let proxy = make_service_fn(|_| async move {
        Ok::<_, hyper::Error>(service_fn(move |req| {
            let client = Client::new();
            handle_request(req, client)
        }))
    });

    let server = server.serve(proxy).with_graceful_shutdown(shutdown_handler);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}

#[cfg(not(target_os = "macos"))]
fn get_activation_socket() -> Result<TcpListener, failure::Error> {
    let mut listenfd = listenfd::ListenFd::from_env();
    listenfd
        .take_tcp_listener(0)?
        .ok_or_else(|| failure::err_msg("No socket provided"))
}

#[cfg(target_os = "macos")]
mod launchd;
#[cfg(target_os = "macos")]
fn get_activation_socket() -> Result<TcpListener, failure::Error> {
    let result = launchd::get_activation_socket("HttpSocket");

    result.map_err(|e| e.into())
}

async fn handle_request(
    mut request: Request<Body>,
    client: Client<HttpConnector>,
) -> Result<Response<Body>, hyper::Error> {
    let host = request.headers().get("HOST").unwrap().to_str().unwrap();
    eprintln!("Serving request for host {:?}", host);
    eprintln!("Full req URI {}", request.uri());

    let app = {
        let process_manager = ProcessManager::global().read().await;

        match process_manager.find_app(&host) {
            Some(app) => app.clone(),
            None => {
                return Ok(host_missing::missing_host_response(host, &process_manager));
            }
        }
    };

    if meta_server::is_meta_request(&request) {
        return Ok(meta_server::handle_request(request, app));
    }

    let destination_url = app_url(&app, request.uri());
    *request.uri_mut() = destination_url;

    // Apply header overrides from config
    request.headers_mut().extend(app.headers().clone());

    let result = client.request(request).await;

    match result {
        Ok(response) => {
            eprintln!("Proxying response");

            Ok(response)
        }
        Err(e) => Ok(error_response(&e, &app)),
    }
}

fn build_address(config: &Config) -> SocketAddr {
    let port = config.general.proxy_port;
    format!("127.0.0.1:{}", port).parse().unwrap()
}

fn app_url(process: &App, request_url: &Uri) -> Uri {
    let base_url = Url::parse("http://localhost/").unwrap();

    let mut destination_url = base_url
        .join(request_url.path_and_query().unwrap().as_str())
        .expect("Invalid request URL");

    destination_url.set_port(Some(process.port())).unwrap();

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
            apps: Vec::new(),
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
            directory: "".to_string(),
            port: Some(42),
            headers: Default::default(),
        };
        let app = App::from_config(&config, 0, "test".to_string());
        let source_uri = "http://testapp.test/path?query=true".parse().unwrap();

        let result = app_url(&app, &source_uri);

        assert_eq!(result, "http://localhost:42/path?query=true")
    }
}
