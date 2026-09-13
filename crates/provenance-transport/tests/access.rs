#![cfg(feature = "test-fixture")]
#[path = "support/records.rs"]
mod records;
use records::{call, get_call, host, Repository};
use serde_json::json;

#[tokio::test]
async fn denied_target_and_unknown_scope_do_not_prepare_projection() {
    let first = Repository::new("The first graph is selected.");
    let second = Repository::new("The second graph is selected.");
    let before_first = first.bytes();
    let before_second = second.bytes();
    let host = host(&[("first", &first), ("second", &second)], &["first"]);
    for (target, scope, kind) in [
        ("second", "default", "access_denied"),
        ("missing", "default", "unknown_target"),
        ("first", "other", "unknown_scope"),
        ("first", "forbidden", "access_denied"),
        ("/tmp", "default", "unknown_target"),
    ] {
        let (_, refused) = call(&host, "get", get_call(target, scope)).await;
        assert_eq!(refused["error"]["kind"], kind, "{refused}");
        assert_eq!(first.bytes(), before_first);
        assert_eq!(second.bytes(), before_second);
    }
    host.shutdown().await;
}

#[tokio::test]
async fn credential_host_and_origin_checks_precede_body_decode_and_storage() {
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;
    let repo = Repository::new("The graph is selected.");
    let before = repo.bytes();
    let host = host(&[("first", &repo)], &["first"]);
    for (credential, authority, origin, kind) in [
        ("", "fixture.test", None, "unauthenticated"),
        ("Bearer invalid", "fixture.test", None, "unauthenticated"),
        ("Bearer fixture-secret", "evil.test", None, "access_denied"),
        (
            "Bearer fixture-secret",
            "fixture.test",
            Some("https://evil.test"),
            "access_denied",
        ),
    ] {
        let mut request = Request::post("/v9/operations/get")
            .header("host", authority)
            .header("authorization", credential);
        if let Some(origin) = origin {
            request = request.header("origin", origin);
        }
        let response = host
            .router()
            .oneshot(request.body(Body::from("invalid json")).unwrap())
            .await
            .unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(value["error"]["kind"], kind);
        assert_eq!(repo.bytes(), before);
    }
    host.shutdown().await;
}

#[tokio::test]
async fn request_roots_and_nested_versions_cannot_bypass_target_selection() {
    let repo = Repository::new("The graph is selected.");
    let before = repo.bytes();
    let host = host(&[("first", &repo)], &["first"]);
    let mut request = get_call("first", "default");
    request["request"]["protocol_version"] = json!(6);
    let (_, refusal) = call(&host, "get", request).await;
    assert_eq!(refusal["error"]["kind"], "protocol_mismatch");
    let mut request = get_call("first", "default");
    request["context"]["root"] = json!(repo.dir.path().to_str().unwrap());
    let (_, refusal) = call(&host, "get", request).await;
    assert_eq!(refusal["error"]["kind"], "invalid_input");
    assert_eq!(repo.bytes(), before);
    host.shutdown().await;
}
