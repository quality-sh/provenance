#![cfg(feature = "test-fixture")]

//! Route classification on the router fallbacks stays behind host
//! authentication: an unknown path and a known path with an unsupported
//! method answer 404 and 405 only to an authenticated caller with an
//! applicable Host and Origin.

use axum::{body::Body, http::Request};
use provenance_transport::StatementHost;
use serde_json::{Value, json};
use tower::ServiceExt as _;

const VALID: &[(&str, &str)] = &[
    ("host", "fixture.test"),
    ("authorization", "Bearer fixture-secret"),
];

fn host() -> StatementHost {
    use provenance_transport::fixture::FixtureAccess;
    let access =
        FixtureAccess::new(Vec::new(), Vec::new(), "fixture-secret", "fixture.test").unwrap();
    StatementHost::with_fixture_access(access)
}

async fn send(
    host: &StatementHost,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
) -> (u16, Value) {
    let mut request = Request::builder().method(method).uri(path);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let response = host
        .router()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let value: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    (status, value)
}

#[tokio::test]
async fn unknown_path_without_credentials_stays_unclassified() {
    let (status, failure) = send(&host(), "GET", "/no-such-route", &[]).await;
    assert_eq!(status, 401, "{failure}");
    assert_eq!(failure["error"]["kind"], "unauthenticated", "{failure}");
    assert_eq!(failure["meta"], json!({}), "{failure}");
}

#[tokio::test]
async fn unknown_path_with_wrong_credentials_stays_unclassified() {
    let headers = &[("host", "fixture.test"), ("authorization", "Bearer wrong")];
    let (status, failure) = send(&host(), "GET", "/no-such-route", headers).await;
    assert_eq!(status, 401, "{failure}");
    assert_eq!(failure["error"]["kind"], "unauthenticated", "{failure}");
    assert_eq!(failure["meta"], json!({}), "{failure}");
}

#[tokio::test]
async fn unknown_path_with_valid_credentials_keeps_unknown_operation() {
    let (status, failure) = send(&host(), "GET", "/no-such-route", VALID).await;
    assert_eq!(status, 404, "{failure}");
    assert_eq!(failure["error"]["kind"], "unknown_operation", "{failure}");
    assert_eq!(failure["meta"], json!({}), "{failure}");
}

#[tokio::test]
async fn unsupported_method_without_credentials_stays_unclassified() {
    let (status, failure) = send(&host(), "DELETE", "/metadata", &[]).await;
    assert_eq!(status, 401, "{failure}");
    assert_eq!(failure["error"]["kind"], "unauthenticated", "{failure}");
    assert_eq!(failure["meta"], json!({}), "{failure}");
}

#[tokio::test]
async fn unsupported_method_with_valid_credentials_keeps_method_not_allowed() {
    let (status, failure) = send(&host(), "DELETE", "/metadata", VALID).await;
    assert_eq!(status, 405, "{failure}");
    assert_eq!(failure["error"]["kind"], "method_not_allowed", "{failure}");
    assert_eq!(failure["meta"], json!({}), "{failure}");
}

#[tokio::test]
async fn unknown_path_with_foreign_host_is_denied_before_classification() {
    let headers = &[("host", "evil.test"), ("authorization", "Bearer fixture-secret")];
    let (status, failure) = send(&host(), "GET", "/no-such-route", headers).await;
    assert_eq!(status, 403, "{failure}");
    assert_eq!(failure["error"]["kind"], "access_denied", "{failure}");
    assert_eq!(failure["meta"], json!({}), "{failure}");
}

#[tokio::test]
async fn unknown_path_with_cross_origin_is_denied_before_classification() {
    let headers = &[
        ("host", "fixture.test"),
        ("authorization", "Bearer fixture-secret"),
        ("origin", "https://evil.test"),
    ];
    let (status, failure) = send(&host(), "GET", "/no-such-route", headers).await;
    assert_eq!(status, 403, "{failure}");
    assert_eq!(failure["error"]["kind"], "access_denied", "{failure}");
    assert_eq!(failure["meta"], json!({}), "{failure}");
}
