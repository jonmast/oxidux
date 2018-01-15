extern crate futures;
extern crate hyper;
extern crate url;

use tokio_core::reactor::Handle;

use futures::future::Future;

use self::url::Url;

use hyper::Client;
use hyper::header::{ContentLength, Host};
use hyper::server::{Request, Response, Service};

pub struct Proxy {
    client: Client<hyper::client::HttpConnector>,
}

impl Proxy {
    pub fn new(handle: Handle) -> Self {
        let client = Client::new(&handle);

        Proxy { client }
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

        let destination_url = build_backend_url(None, request.uri());

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

// TODO: add some error handling for this fn
fn build_backend_url(backend_server_port: Option<u16>, request_path: &hyper::Uri) -> hyper::Uri {
    let base_url = Url::parse("http://localhost/").unwrap();

    let mut destination_url = base_url
        .join(request_path.as_ref())
        .expect("Invalid request URL");

    destination_url.set_port(backend_server_port).unwrap();

    println!("Starting request to backend {}", destination_url);

    destination_url.as_str().parse().unwrap()
}

const ERROR_MESSAGE: &'static str = "No response from server";

fn error_response(error: hyper::Error) -> Response {
    println!("Request to backend failed with error \"{}\"", error);
    Response::new()
        .with_header(ContentLength(ERROR_MESSAGE.len() as u64))
        .with_body(ERROR_MESSAGE)
}
