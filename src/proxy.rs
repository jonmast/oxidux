use hyper;
use futures;

use tokio_core::reactor::Handle;

use futures::future::Future;

use hyper::Client;
use hyper::header::{ContentLength, Host};
use hyper::server::{Request, Response, Service};

use process_manager::ProcessManager;

pub struct Proxy {
    client: Client<hyper::client::HttpConnector>,
    process_manager: ProcessManager,
}

impl Proxy {
    pub fn new(handle: Handle, process_manager: ProcessManager) -> Self {
        let client = Client::new(&handle);

        Proxy {
            client,
            process_manager,
        }
    }
}

impl Service for Proxy {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;

    // The future that the response will go to
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, request: Request) -> Self::Future {
        let host: &Host = request.headers().get().unwrap();
        println!("Serving request for host {:?}", host);
        println!("Full req URI {}", request.uri());

        let destination_url = match self.process_manager.find_process(&host) {
            Some(process) => process.url(request.uri()),
            None => return Box::new(futures::future::ok(missing_host_response())),
        };

        // let destination_url = build_backend_url(None, request.uri());

        Box::new(
            self.client
                .get(destination_url)
                .then(|result| match result {
                    Ok(response) => {
                        println!("Proxying response");
                        futures::future::ok(
                            Response::new()
                                .with_headers(response.headers().clone())
                                .with_body(response.body()),
                        )
                    }
                    Err(e) => futures::future::ok(error_response(e)),
                }),
        )
    }
}

const ERROR_MESSAGE: &'static str = "No response from server";

fn error_response(error: hyper::Error) -> Response {
    println!("Request to backend failed with error \"{}\"", error);
    Response::new()
        .with_header(ContentLength(ERROR_MESSAGE.len() as u64))
        .with_body(ERROR_MESSAGE)
}

const MISSING_HOST_MESSAGE: &'static str = "No such host was found";
fn missing_host_response() -> Response {
    Response::new()
        .with_header(ContentLength(MISSING_HOST_MESSAGE.len() as u64))
        .with_body(MISSING_HOST_MESSAGE)
}
