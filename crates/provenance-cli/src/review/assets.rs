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

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}
