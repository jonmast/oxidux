use futures::StreamExt;
use hyper::{Body, Request, Response, StatusCode};

use crate::app::App;

pub fn handle_request(request: Request<Body>, app: App) -> Response<Body> {
    let action = request.uri().path().split('/').nth(2);

    match action {
        Some("status") => status_response(app),
        Some("logstream") => logstream_response(app),
        _ => not_found_response(),
    }
}

pub fn is_meta_request<T>(request: &Request<T>) -> bool {
    request.uri().path().starts_with("/__oxidux__/")
}

fn status_response(app: App) -> Response<Body> {
    let status = if app.is_running() {
        "Running"
    } else {
        "Stopped"
    };

    Response::new(Body::from(status))
}

fn not_found_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not Found"))
        .unwrap()
}

fn logstream_response(app: App) -> Response<Body> {
    let output = app
        .output_stream()
        .map(|(process, line)| -> Result<_, failure::Error> {
            Ok(format!("data: {}: {}\n\n", process.name(), line))
        });

    Response::builder()
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .body(Body::wrap_stream(output))
        .unwrap()
}
