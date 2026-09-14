#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
}
use axum::{body::Body, http::Request};
use provenance_transport::StatementHost;
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::{json, Value};
use support::records::Repository;
use tower::ServiceExt as _;

fn host(repo: &Repository, writable: bool) -> StatementHost {
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

async fn call(host: &StatementHost, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret");
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

#[tokio::test]
async fn collection_member_query_and_write_use_the_bound_identity() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let (status, metadata) = call(&host, "GET", "/metadata", None).await;
    assert_eq!(status, 200, "{metadata}");
    assert_eq!(metadata["data"]["repository"], "selected");
    assert_eq!(metadata["data"]["scope"], "default");

    let (status, listed) = call(&host, "GET", "/sources", None).await;
    assert_eq!(status, 200, "{listed}");
    assert_eq!(listed["data"]["items"][0]["id"], "source_shared");
    assert_eq!(listed["data"]["items"][0]["scope_id"], "default");

    let (status, member) = call(&host, "GET", "/rules/rule_shared", None).await;
    assert_eq!(status, 200, "{member}");
    assert_eq!(member["data"]["id"], "rule_shared");

    let (status, search) = call(&host, "GET", "/rules?query=search&text=shared", None).await;
    assert_eq!(status, 200, "{search}");
    assert!(search["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["id"] == "rule_shared"));

    let source = json!({"data":{
        "id":"source_created", "name":"Created source", "source_type":"document",
        "url":null, "reference":null, "commit_pin":null, "effective_date":null,
        "review_date":null, "supersedes":[], "origin_thread":null, "origin_message":null
    }});
    let (status, created) = call(&host, "POST", "/sources", Some(source)).await;
    assert_eq!(status, 200, "{created}");
    assert_eq!(created["data"]["scope_id"], "default");
    assert_eq!(created["data"]["id"], "source_created");
}

#[tokio::test]
async fn authentication_host_and_origin_checks_precede_body_decoding() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, false);
    for (credential, authority, origin, status, kind) in [
        ("", "fixture.test", None, 401, "unauthenticated"),
        (
            "Bearer invalid",
            "fixture.test",
            None,
            401,
            "unauthenticated",
        ),
        (
            "Bearer fixture-secret",
            "evil.test",
            None,
            403,
            "access_denied",
        ),
        (
            "Bearer fixture-secret",
            "fixture.test",
            Some("https://evil.test"),
            403,
            "access_denied",
        ),
    ] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/statement-checks")
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
        assert_eq!(response.status().as_u16(), status);
        let value: Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(value["error"]["kind"], kind);
        assert_eq!(value["meta"], json!({}));
    }
}

#[tokio::test]
async fn mcp_keeps_role_subsets_and_returns_the_http_envelope() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, false);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server_host = host.clone();
    let server = tokio::spawn(async move { server_host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    assert!(tools.iter().any(|tool| tool.name == "list-sources"));
    assert!(!tools.iter().any(|tool| tool.name == "create-source"));
    assert!(tools
        .iter()
        .all(|tool| tool.description.as_deref() != Some("Invoke the shared operation.")));
    let result = client
        .call_tool(CallToolRequestParams::new("list-sources"))
        .await
        .unwrap();
    let value = result.structured_content.unwrap();
    assert!(value["data"]["items"].is_array());
    assert!(value["meta"].is_object());
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn requirement_patch_binds_receipt_and_precondition_headers() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let request = Request::builder()
        .method("GET")
        .uri("/requirements/req_shared")
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret")
        .body(Body::empty())
        .unwrap();
    let response = host.router().oneshot(request).await.unwrap();
    let etag = response
        .headers()
        .get("etag")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let request = Request::builder()
        .method("PATCH")
        .uri("/requirements/req_shared")
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret")
        .header("content-type", "application/json")
        .header("idempotency-key", "request_transport_edit")
        .header("if-match", etag)
        .body(Body::from(
            json!({"data":{"actor":"ben","description":"Edited through the resource route."}})
                .to_string(),
        ))
        .unwrap();
    let response = host.router().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let value: Value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(status, 200, "{value}");
    assert_eq!(
        value["data"]["description"],
        "Edited through the resource route."
    );
    assert!(value["data"]["edit"]["etag"].is_string());
    assert!(value["data"]["decision"].is_object());
}
