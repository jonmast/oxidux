use futures::StreamExt;
use hyper::{Body, Request, Response, StatusCode};

use crate::app::App;

pub async fn handle_request(
    request: Request<Body>,
    app: App,
) -> color_eyre::Result<Response<Body>> {
    let action = request.uri().path().split('/').nth(2);

    match action {
        Some("status") => status_response(app).await,
        Some("logstream") => logstream_response(app).await,
        _ => not_found_response(),
    }
}

pub fn is_meta_request<T>(request: &Request<T>) -> bool {
    request.uri().path().starts_with("/__oxidux__/")
}

async fn status_response(app: App) -> color_eyre::Result<Response<Body>> {
    let mut status = "".to_string();

    for process in app.processes {
        status.push_str(&format!(
            "{}: {:?}\n",
            process.name().await,
            process.run_state().await
        ));
    }

    Ok(Response::new(Body::from(status)))
}

fn not_found_response() -> color_eyre::Result<Response<Body>> {
    Ok(Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not Found"))?)
}

async fn logstream_response(app: App) -> color_eyre::Result<Response<Body>> {
    let output = app
        .output_stream()
        .await
        .then(|(process, line)| async move {
            Ok::<_, eyre::Error>(format!("data: {}: {}\n\n", process.name().await, line))
        });

    Ok(Response::builder()
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .body(Body::wrap_stream(output))?)
}
