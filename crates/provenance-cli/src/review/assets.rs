use axum::{
    body::Body,
    extract::Request,
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
};
use provenance_macros::rule;

include!(concat!(env!("OUT_DIR"), "/review_assets.rs"));

/// Answers every asset path the review page asks for from the bundle this
/// package was compiled with. The `include!` above embeds that bundle at
/// compile time, so the bytes handed out are the bytes the package carries.
#[rule("rule_cli_serves_review_assets")]
pub(super) async fn serve(request: Request) -> Response {
    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let path = match request.uri().path() {
        "/" => "/index.html",
        path => path,
    };
    let Some((_, bytes)) = ASSETS.iter().find(|(candidate, _)| *candidate == path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let body = if request.method() == Method::HEAD {
        Body::empty()
    } else {
        Body::from(*bytes)
    };
    Response::builder()
        .header("content-type", content_type(path))
        .header("content-length", bytes.len())
        .body(body)
        .expect("static asset headers")
}

/// The media type for each file extension that the review bundle can hold.
const CONTENT_TYPES: &[(&str, &str)] = &[
    ("html", "text/html; charset=utf-8"),
    ("js", "text/javascript; charset=utf-8"),
    ("mjs", "text/javascript; charset=utf-8"),
    ("css", "text/css; charset=utf-8"),
    ("json", "application/json"),
    ("map", "application/json"),
    ("svg", "image/svg+xml"),
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("webp", "image/webp"),
    ("gif", "image/gif"),
    ("ico", "image/x-icon"),
    ("woff", "font/woff"),
    ("woff2", "font/woff2"),
    ("ttf", "font/ttf"),
    ("wasm", "application/wasm"),
];

fn content_type(path: &str) -> &'static str {
    let extension = path.rsplit('.').next().unwrap_or_default();
    CONTENT_TYPES
        .iter()
        .find(|(candidate, _)| *candidate == extension)
        .map_or("application/octet-stream", |&(_, media_type)| media_type)
}

#[cfg(test)]
#[path = "assets_tests.rs"]
mod tests;
