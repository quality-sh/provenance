//! Pins the typed failure reasons produced while decoding a request body.
//!
//! A body that is not valid JSON reports `malformed_json`; a body that is
//! valid JSON but not a valid `{ "data": ... }` envelope reports
//! `invalid_value`. Both refusals use status 400 with the common failure
//! envelope.

use axum::{body::Body, http::Request};
use provenance_transport::StatementHost;
use serde_json::Value;
use tower::ServiceExt as _;

async fn post_raw(host: &StatementHost, path: &str, body: &str) -> (u16, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body.as_bytes().to_vec()))
        .unwrap();
    let response = host.router().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn malformed_json_body_reports_the_malformed_json_reason() {
    let host = StatementHost::default();
    let (status, failure) = post_raw(&host, "/statement-checks", r#"{"data":{"statement":"#).await;
    assert_eq!(status, 400, "{failure}");
    assert_eq!(failure["error"]["kind"], "invalid_input", "{failure}");
    assert_eq!(failure["error"]["reason"], "malformed_json", "{failure}");
}

#[tokio::test]
async fn well_formed_body_with_invalid_envelope_keeps_the_invalid_value_reason() {
    let host = StatementHost::default();
    for body in ["[1,2,3]", r#"{"no_data":true}"#] {
        let (status, failure) = post_raw(&host, "/statement-checks", body).await;
        assert_eq!(status, 400, "{failure}");
        assert_eq!(failure["error"]["kind"], "invalid_input", "{failure}");
        assert_eq!(failure["error"]["reason"], "invalid_value", "{failure}");
    }
}
