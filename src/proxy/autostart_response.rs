use hyper::{Body, Response};

const RESTART_RESPONSE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/static/restart_response.html"
));

pub fn autostart_response() -> Response<Body> {
    let body = Body::from(RESTART_RESPONSE);

    Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .body(body)
        .unwrap()
}
