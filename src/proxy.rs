use futures;
use hyper;

use futures::future::Future;

use hyper::client::HttpConnector;
use hyper::Client;
use hyper::{Body, Request, Response};

use process_manager::ProcessManager;

const ERROR_MESSAGE: &'static str = "No response from server";

fn error_response(error: hyper::Error) -> Response<Body> {
    println!("Request to backend failed with error \"{}\"", error);

    let body = Body::from(ERROR_MESSAGE);
    Response::new(body)
}

const MISSING_HOST_MESSAGE: &'static str = "No such host was found";
fn missing_host_response() -> Response<Body> {
    let body = Body::from(MISSING_HOST_MESSAGE);

    Response::new(body)
}

pub fn handle_request(
    request: Request<Body>,
    client: &Client<HttpConnector>,
    process_manager: &ProcessManager,
) -> Box<Future<Item = Response<Body>, Error = hyper::Error> + Send> {
    let host = request.headers().get("HOST").unwrap().to_str().unwrap();
    println!("Serving request for host {:?}", host);
    println!("Full req URI {}", request.uri());

    let destination_url = match process_manager.find_process(&host) {
        Some(process) => process.url(request.uri()),
        None => return Box::new(futures::future::ok(missing_host_response())),
    };

    Box::new(client.get(destination_url).then(|result| match result {
        Ok(response) => {
            println!("Proxying response");
            let (parts, body) = response.into_parts();

            futures::future::ok(Response::from_parts(parts, body))
        }
        Err(e) => futures::future::ok(error_response(e)),
    }))
}
