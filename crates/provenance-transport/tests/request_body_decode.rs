//! Pins the typed failure reasons produced while decoding a request body.
//!
//! A body that is not valid JSON reports `malformed_json`; a body that is
//! valid JSON but not a valid `{ "data": ... }` envelope reports
//! `invalid_value`. Both refusals use status 400 with the common failure
//! envelope.

use axum::{body::Body, http::Request};
use provenance_macros::verifies;
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
#[verifies("rule_http_request_decode_reasons", examples)]
async fn malformed_json_body_reports_the_malformed_json_reason() {
    let host = StatementHost::default();
    let (status, failure) = post_raw(&host, "/statement-checks", r#"{"data":{"statement":"#).await;
    assert_eq!(status, 400, "{failure}");
    assert_eq!(failure["error"]["kind"], "invalid_input", "{failure}");
    assert_eq!(failure["error"]["reason"], "malformed_json", "{failure}");
}

#[tokio::test]
#[verifies("rule_http_request_decode_reasons", examples)]
async fn well_formed_body_with_invalid_envelope_keeps_the_invalid_value_reason() {
    let host = StatementHost::default();
    for body in ["[1,2,3]", r#"{"no_data":true}"#] {
        let (status, failure) = post_raw(&host, "/statement-checks", body).await;
        assert_eq!(status, 400, "{failure}");
        assert_eq!(failure["error"]["kind"], "invalid_input", "{failure}");
        assert_eq!(failure["error"]["reason"], "invalid_value", "{failure}");
    }
}

#[cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}

/// e92s: a create that carries `source_refs` names the field, lists the
/// accepted fields and tells how to cite a Source.
#[cfg(feature = "test-fixture")]
#[tokio::test]
#[verifies("rule_unknown_request_field_names_accepted_fields", examples)]
async fn an_unknown_request_field_names_itself_and_the_accepted_fields() {
    use support::resource_http::{call, host};
    let repo = support::records::Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let body = serde_json::json!({"data":{
        "actor":"reviewer", "id":"req_new", "statement":"The system stores each record.",
        "status":"discovery", "depends_on":[], "supersedes":[], "source_refs":[]
    }});
    let (status, failure) = call(&host, "POST", "/requirements", Some(body)).await;
    assert_eq!(status, 400, "{failure}");
    let error = &failure["error"];
    assert_eq!(error["kind"], "invalid_input", "{failure}");
    assert_eq!(error["field"], "request.source_refs", "{failure}");
    assert_eq!(error["reason"], "unknown_field", "{failure}");
    let message = error["message"].as_str().unwrap();
    assert!(
        message.starts_with("create-requirement does not take source_refs; accepted fields: "),
        "{message}"
    );
    for field in ["`actor`", "`statement`", "`depends_on`"] {
        assert!(message.contains(field), "{message}");
    }
    assert!(
        message.ends_with(
            "; to cite a Source, update the record with the Source in relationships.cites"
        ),
        "{message}"
    );
}
