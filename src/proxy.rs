use futures::future::{self, Future};

use hyper;
use hyper::client::HttpConnector;
use hyper::{Body, Client, Request, Response};
mod autostart_response;
mod host_missing;

use crate::{process::Process, process_manager::ProcessManager};

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

pub fn handle_request(
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
