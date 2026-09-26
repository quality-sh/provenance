use axum::{body::Body, http::Request};
use provenance_transport::StatementHost;
use serde_json::Value;
use tower::ServiceExt as _;

use super::records::Repository;

pub fn host(repo: &Repository, writable: bool) -> StatementHost {
    use provenance_transport::fixture::{FixtureAccess, Target};
    let access = FixtureAccess::new(
        vec![Target {
            id: "selected".into(),
            root: repo.dir.path().to_path_buf(),
        }],
        vec![("selected".into(), "default".into())],
        "fixture-secret",
        "fixture.test",
    )
    .unwrap();
    StatementHost::with_fixture_access(if writable {
        access.allow_writes()
    } else {
        access
    })
}

pub async fn call(
    host: &StatementHost,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (u16, Value) {
    call_with_headers(host, method, path, body, &[]).await
}

#[allow(clippy::option_if_let_else)]
pub async fn call_with_headers(
    host: &StatementHost,
    method: &str,
    path: &str,
    body: Option<Value>,
    headers: &[(&str, &str)],
) -> (u16, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret");
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let body = if let Some(value) = body {
        request = request.header("content-type", "application/json");
        Body::from(value.to_string())
    } else {
        Body::empty()
    };
    let response = host
        .router()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    (status, value)
}
