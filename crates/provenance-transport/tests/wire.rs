use axum::{body::Body, http::Request};
use provenance_transport::StatementHost;
use serde_json::{json, Value};
use tower::ServiceExt as _;

#[allow(clippy::option_if_let_else)]
async fn request(
    host: &StatementHost,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (u16, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    let body = match body {
        Some(value) => {
            builder = builder.header("content-type", "application/json");
            Body::from(serde_json::to_vec(&value).unwrap())
        }
        None => Body::empty(),
    };
    let response = host
        .router()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn http_uses_resource_routes_and_the_single_envelope() {
    let host = StatementHost::default();
    let (status, value) = request(
        &host,
        "POST",
        "/statement-checks",
        Some(json!({"data":{"statement":"The system stores records."}})),
    )
    .await;
    assert_eq!(status, 200, "{value}");
    assert!(value["data"].is_object());
    assert_eq!(value["meta"], json!({}));
    assert!(value.get("protocol_version").is_none());
    assert!(value.get("operation").is_none());

    let (status, failure) = request(
        &host,
        "POST",
        "/v9/operations/check-statement",
        Some(json!({"data":{"statement":"x"}})),
    )
    .await;
    assert_eq!(status, 404);
    assert_eq!(failure["error"]["kind"], "unknown_operation");
    assert_eq!(failure["meta"], json!({}));
}

#[tokio::test]
async fn metadata_carries_the_tuple_in_an_envelope() {
    let host = StatementHost::default();
    let (status, value) = request(&host, "GET", "/metadata", None).await;
    assert_eq!(status, 200);
    assert_eq!(
        value["data"]["compatibility"],
        json!({
            "wire": 9, "state": 2, "review_journal": 3, "read_derivation": 3
        })
    );
    // No contract digest: the tuple, the package identity, and the bound
    // repository and scope are the whole advertisement.
    assert!(value["data"].get("contract_digest").is_none());
    assert_eq!(value["meta"], json!({}));
}
