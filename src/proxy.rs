use futures::future::{self, Future};
use hyper::service::service_fn;
use std::net::SocketAddr;

use hyper;
use hyper::client::HttpConnector;
use hyper::{Body, Client, Request, Response, Server};

mod autostart_response;
mod host_missing;

use crate::{config::Config, process::Process, process_manager::ProcessManager};

const ERROR_MESSAGE: &str = "No response from server";

fn error_response(error: &hyper::Error, process: &Process) -> Response<Body> {
    eprintln!("Request to backend failed with error \"{}\"", error);

    if process.is_running() {
        let body = Body::from(ERROR_MESSAGE);
        Response::new(body)
    } else {
        process
            .start()
            .unwrap_or_else(|e| eprint!("Failed to auto-start app, got {}", e));

        autostart_response::autostart_response()
    }
}

pub fn start_server(
    config: &Config,
    process_manager: ProcessManager,
    shutdown_handler: impl Future,
) -> impl Future<Item = (), Error = ()> {
    let mut listenfd = listenfd::ListenFd::from_env();
    let (addr, server) = if let Some(listener) = listenfd.take_tcp_listener(0).unwrap() {
        let addr = listener.local_addr().unwrap();
        (addr, Server::from_tcp(listener).unwrap())
    } else {
        let addr = &build_address(&config);
        (*addr, Server::bind(addr))
    };

    eprintln!("Starting proxy server on {}", addr);

    let proxy = move || {
        let client = Client::new();
        let process_manager = process_manager.clone();

        service_fn(move |req| handle_request(req, &client, &process_manager))
    };
    server
        .serve(proxy)
        .with_graceful_shutdown(shutdown_handler.and_then(|_| Ok(())))
        .map_err(|err| eprintln!("serve error: {:?}", err))
}

fn handle_request(
    mut request: Request<Body>,
    client: &Client<HttpConnector>,
    process_manager: &ProcessManager,
) -> Box<future::Future<Item = Response<Body>, Error = hyper::Error> + Send> {
    let host = request.headers().get("HOST").unwrap().to_str().unwrap();
    eprintln!("Serving request for host {:?}", host);
    eprintln!("Full req URI {}", request.uri());

    let process = match process_manager.find_process(&host) {
        Some(process) => process.clone(),
        None => {
            return Box::new(futures::future::ok(host_missing::missing_host_response(
                host,
                process_manager,
            )));
        }
    };

    let destination_url = process.url(request.uri());
    *request.uri_mut() = destination_url;

    // Apply header overrides from config
    request.headers_mut().extend(process.headers());

    Box::new(client.request(request).then(move |result| match result {
        Ok(response) => {
            eprintln!("Proxying response");

            future::ok(response)
        }
        Err(e) => future::ok(error_response(&e, &process)),
    }))
}

fn build_address(config: &Config) -> SocketAddr {
    let port = config.general.proxy_port;
    format!("127.0.0.1:{}", port).parse().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_bind_address_from_config() {
        let config = config::Config {
            apps: Vec::new(),
            general: config::ProxyConfig { proxy_port: 80 },
        };

        let addr = build_address(&config);

        assert_eq!(addr.port(), 80);
        let localhost: std::net::IpAddr = "127.0.0.1".parse().unwrap();
        assert_eq!(addr.ip(), localhost);
    }
}
