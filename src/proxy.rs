use futures::future::{self, Future};

use hyper;
use hyper::client::HttpConnector;
use hyper::Client;
use hyper::{Body, Request, Response};

use crate::process_manager::ProcessManager;

const ERROR_MESSAGE: &str = "No response from server";

fn error_response(error: &hyper::Error) -> Response<Body> {
    eprintln!("Request to backend failed with error \"{}\"", error);

    let body = Body::from(ERROR_MESSAGE);
    Response::new(body)
}

const MISSING_HOST_MESSAGE: &str = "No such host was found";
fn missing_host_response() -> Response<Body> {
    let body = Body::from(MISSING_HOST_MESSAGE);

    Response::new(body)
}

pub fn handle_request(
    request: &Request<Body>,
    client: &Client<HttpConnector>,
    process_manager: &ProcessManager,
) -> Box<future::Future<Item = Response<Body>, Error = hyper::Error> + Send> {
    let host = request.headers().get("HOST").unwrap().to_str().unwrap();
    eprintln!("Serving request for host {:?}", host);
    eprintln!("Full req URI {}", request.uri());

    let destination_url = match process_manager.find_process(&host) {
        Some(process) => process.url(request.uri()),
        None => return Box::new(futures::future::ok(missing_host_response())),
    };

    Box::new(client.get(destination_url).then(|result| match result {
        Ok(response) => {
            eprintln!("Proxying response");
            let (parts, body) = response.into_parts();

            future::ok(Response::from_parts(parts, body))
        }
        Err(e) => future::ok(error_response(&e)),
    }))
}
