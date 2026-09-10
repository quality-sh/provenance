#![cfg(feature = "test-fixture")]
#[path = "support/records.rs"]
mod records;
use provenance_core::protocol::{QueryResponse, ReadDocumentQuery, SearchQuery};
use provenance_core::ScopeId;
use provenance_store::operations::{queries, read_policy::ReadPolicy};
use records::{call, host, Repository};
use rmcp::{model::CallToolRequestParams, ServiceExt};
use serde_json::{json, Value};
use std::fmt::Write as _;

fn seed(repo: &Repository) {
    let path = provenance_store::shards::requirements_path(
        &repo.layout,
        &ScopeId::new("default").unwrap(),
    );
    let original = std::fs::read_to_string(&path).unwrap();
    let mut row: Value = serde_json::from_str(original.lines().next().unwrap()).unwrap();
    let mut body = original;
    for i in 0..205 {
        row["id"] = json!(format!("req_child_{i:03}"));
        row["refines"] = json!("req_shared");
        writeln!(body, "{row}").unwrap();
    }
    std::fs::write(path, body).unwrap();
}
fn input(request: &Value) -> Value {
    json!({"context":{"repository":"first","scope":"default","freshness":"catch_up"},"request":request})
}

#[tokio::test]
async fn native_http_and_mcp_share_cursor_pages_and_refusals() {
    let repo = Repository::new("The graph is readable.");
    seed(&repo);
    repo.add_scope("other", "The other scope is readable.");
    let second = Repository::new("The second graph is readable.");
    let host = host(
        &[("first", &repo), ("second", &second)],
        &["first", "second"],
    );
    assert_eq!(
        call(&host, "get", records::get_call("first", "default"))
            .await
            .0,
        200
    );
    let (client_io, server_io) = tokio::io::duplex(1024 * 1024);
    let serving = host.clone();
    let server = tokio::spawn(async move { serving.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    for (operation, mut request, field) in [
        (
            "search",
            json!({"text":"req_","node_types":["requirement"],"limit":200}),
            "nodes",
        ),
        (
            "read-document",
            json!({"id":"req_shared","limit":200}),
            "entries",
        ),
    ] {
        let (status, first) = call(&host, operation, input(&request)).await;
        assert_eq!(status, 200, "{first}");
        assert_eq!(first[field].as_array().unwrap().len(), 200);
        let native = native_page(&repo, operation, &request).await;
        assert_eq!(first, native);
        request["cursor"] = first["next_cursor"].clone();
        let continuation = input(&request);
        let (status, next) = call(&host, operation, continuation.clone()).await;
        assert_eq!(status, 200, "{next}");
        assert!(next["next_cursor"].is_null());
        let params = CallToolRequestParams::new(operation).with_arguments(
            json!({"protocol_version":8,"call":continuation})
                .as_object()
                .unwrap()
                .clone(),
        );
        let mcp = client.call_tool(params).await.unwrap();
        assert_eq!(mcp.structured_content.unwrap(), next);
        for (field, value) in [("scope", "other"), ("repository", "second")] {
            let mut foreign = input(&request);
            foreign["context"][field] = json!(value);
            let (status, refusal) = call(&host, operation, foreign).await;
            assert_eq!(status, 409, "{refusal}");
            assert_eq!(refusal["error"]["kind"], "cursor_invalid");
        }
        let (other_operation, other_request) = if operation == "search" {
            (
                "read-document",
                json!({"id":"req_shared","limit":200,"cursor":first["next_cursor"]}),
            )
        } else {
            (
                "search",
                json!({"text":"req_","limit":200,"cursor":first["next_cursor"]}),
            )
        };
        let (status, refused) = call(&host, other_operation, input(&other_request)).await;
        assert_eq!(status, 409);
        assert_eq!(refused["error"]["kind"], "cursor_invalid");
        let mut bad = request.clone();
        bad["cursor"] = json!(format!("{}x", bad["cursor"].as_str().unwrap()));
        let (status, refusal) = call(&host, operation, input(&bad)).await;
        assert_eq!(status, 409);
        assert_eq!(refusal["error"]["kind"], "cursor_invalid");
        let denied = host_with_no_grant(&repo);
        let before = repo.bytes();
        assert_eq!(call(&denied, operation, input(&request)).await.0, 403);
        assert_eq!(repo.bytes(), before);
        denied.shutdown().await;
        repo.edit("default", &format!("The {operation} revision is changed."));
        let (status, refusal) = call(&host, operation, input(&request)).await;
        assert_eq!(status, 409, "{refusal}");
        assert_eq!(refusal["error"]["kind"], "cursor_revision_changed");
    }
    client.cancel().await.unwrap();
    server.await.unwrap();
    host.shutdown().await;
}
fn host_with_no_grant(repo: &Repository) -> provenance_transport::StatementHost {
    host(&[("first", repo)], &[])
}

async fn native_page(repo: &Repository, operation: &str, request: &Value) -> Value {
    let root = repo.dir.path().to_str().unwrap().into();
    let scope = ScopeId::new("default").unwrap();
    if operation == "search" {
        serde_json::to_value(QueryResponse::new(
            "search",
            queries::search(
                Some(root),
                &scope,
                ReadPolicy::default(),
                serde_json::from_value::<SearchQuery>(request.clone()).unwrap(),
            )
            .await
            .unwrap(),
        ))
        .unwrap()
    } else {
        serde_json::to_value(QueryResponse::new(
            "read-document",
            queries::read_document(
                Some(root),
                &scope,
                ReadPolicy::default(),
                serde_json::from_value::<ReadDocumentQuery>(request.clone()).unwrap(),
            )
            .await
            .unwrap(),
        ))
        .unwrap()
    }
}
