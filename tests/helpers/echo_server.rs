use hyper::body::Buf;
/// Helper binary for testing
///
/// This boots up a basic http server that echos back the request body and headers, which is useful
/// for testing purposes
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use serde::Serialize;
use std::env;
use std::{convert::Infallible, net::SocketAddr};

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let port = env::var("PORT")?.parse()?;
    // Test server that we're proxying to
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let make_svc = make_service_fn(|_conn| async { Ok::<_, Infallible>(service_fn(handle)) });

    let server = Server::bind(&addr).serve(make_svc);

    println!("Running");

    if let Err(e) = server.await {
        eprintln!("Error spawning test server: {}", e);
    }

    Ok(())
}

#[derive(Serialize)]
struct EchoResponse {
    url: String,
    headers: std::collections::HashMap<String, String>,
    body: String,
}

impl EchoResponse {
    async fn from_request(request: &mut Request<Body>) -> Self {
        let url = request.uri().to_string();
        let headers = request
            .headers()
            .iter()
            .map(|(name, value)| (name.as_str().into(), value.to_str().unwrap().into()))
            .collect();
        let body = String::from_utf8(
            hyper::body::aggregate(request)
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();

        Self { url, headers, body }
    }
}

async fn handle(mut request: Request<Body>) -> color_eyre::Result<Response<Body>> {
    let response_json = serde_json::to_string(&EchoResponse::from_request(&mut request).await)?;
    Ok(Response::new(Body::from(response_json)))
}
