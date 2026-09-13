use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use provenance_transport::StatementHost;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn call(host: &StatementHost, path: &str, body: &str) -> (StatusCode, Value) {
    let response = host
        .router()
        .oneshot(
            Request::post(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn http_statement_reports_match_analyzer_without_repository_access() {
    let host = StatementHost::default();
    for statement in ["Install the cover.", "Stop; wait.", "Café; wait.", ""] {
        let (status, actual) = call(
            &host,
            "/v9/operations/check-statement",
            &json!({
                "request": {"statement": statement}
            })
            .to_string(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            actual,
            serde_json::to_value(provenance_ste100::check_descriptive(statement)).unwrap()
        );
    }
    host.shutdown().await;
}

#[tokio::test]
async fn invalid_statement_calls_have_typed_refusals() {
    let host = StatementHost::default();
    for body in [
        "{",
        "{}",
        r#"{"request":{}}"#,
        r#"{"request":{"statement":null}}"#,
        r#"{"request":{"statement":12}}"#,
        r#"{"request":{"statement":"x","extra":1}}"#,
        r#"{"request":{"statement":"x"},"context":{"repository":"outside"}}"#,
    ] {
        let (status, actual) = call(&host, "/v9/operations/check-statement", body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}: {actual}");
        assert_eq!(actual["error"]["kind"], "invalid_input");
    }
    host.shutdown().await;
}

#[tokio::test]
async fn protocol_and_unknown_operation_refusals_preserve_dispatch_identity() {
    let host = StatementHost::default();
    let (status, failure) = call(
        &host,
        "/v6/operations/check-statement",
        r#"{"request":{"statement":"x"}}"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        failure["error"],
        json!({"kind":"protocol_mismatch", "requested":6,"supported":9})
    );
    let (status, failure) = call(&host, "/v9/operations/not-an-operation", "{}").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(failure["error"]["kind"], "unknown_operation");
    assert!(failure.get("operation").is_none());
}

#[tokio::test]
async fn metadata_and_statement_calls_leave_fixture_tree_untouched() {
    let directory = tempfile::tempdir().unwrap();
    let sentinel = directory.path().join("sentinel");
    std::fs::write(&sentinel, b"unchanged").unwrap();
    let host = StatementHost::default();
    let response = host
        .router()
        .oneshot(Request::get("/metadata").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let metadata: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(metadata["protocol_version"], 9);
    assert_eq!(metadata["engine_version"], env!("CARGO_PKG_VERSION"));
    assert!(metadata.get("repository").is_none());
    call(
        &host,
        "/v9/operations/check-statement",
        r#"{"request":{"statement":"Close the valve."}}"#,
    )
    .await;
    host.shutdown().await;
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"unchanged");
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[tokio::test]
async fn oversized_http_body_refuses_before_analyzer() {
    let host = StatementHost::default();
    let body = json!({"request":{"statement":"a".repeat(2 * 1024 * 1024)}}).to_string();
    let (status, refusal) = call(&host, "/v9/operations/check-statement", &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(refusal["error"]["reason"], "too_large");
    host.shutdown().await;
}

#[tokio::test]
async fn request_admission_bounds_incomplete_bodies_before_decode() {
    let host = StatementHost::default();
    let mut readers = Vec::new();
    for _ in 0..8 {
        let (started, ready) = tokio::sync::oneshot::channel();
        let mut started = Some(started);
        let pending = futures::stream::poll_fn(move |_| {
            if let Some(started) = started.take() {
                let _ = started.send(());
            }
            std::task::Poll::<Option<Result<axum::body::Bytes, std::convert::Infallible>>>::Pending
        });
        let router = host.router();
        readers.push(tokio::spawn(async move {
            router
                .oneshot(
                    Request::post("/v9/operations/check-statement")
                        .body(Body::from_stream(pending))
                        .unwrap(),
                )
                .await
                .unwrap()
        }));
        ready.await.unwrap();
    }
    let (status, failure) = call(&host, "/v9/operations/check-statement", "{}").await;
    for reader in readers {
        reader.abort();
        let _ = reader.await;
    }
    host.shutdown().await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(failure["error"]["kind"], "unavailable_needs");
}

#[tokio::test]
async fn shutdown_releases_incomplete_http_body_without_waiting_for_peer() {
    let host = StatementHost::default();
    let (started, ready) = tokio::sync::oneshot::channel();
    let mut started = Some(started);
    let pending = futures::stream::poll_fn(move |_| {
        if let Some(started) = started.take() {
            let _ = started.send(());
        }
        std::task::Poll::<Option<Result<axum::body::Bytes, std::convert::Infallible>>>::Pending
    });
    let router = host.router();
    let reader = tokio::spawn(async move {
        router
            .oneshot(
                Request::post("/v9/operations/check-statement")
                    .body(Body::from_stream(pending))
                    .unwrap(),
            )
            .await
            .unwrap()
    });
    ready.await.unwrap();
    host.shutdown().await;
    let response = tokio::time::timeout(std::time::Duration::from_secs(2), reader).await;
    assert!(
        response.is_ok(),
        "Shutdown must release a body that has not completed"
    );
    assert_eq!(
        response.unwrap().unwrap().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}
