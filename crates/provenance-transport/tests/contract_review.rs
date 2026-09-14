#![cfg(feature = "test-fixture")]
#[allow(dead_code)]
#[path = "support/records.rs"]
mod records;
#[path = "support/writes.rs"]
mod writes;

use provenance_core::{ScopeId, StableId};
use provenance_store::{review::SaveRequirement, state_store::StateStore};
use records::{call, host, Repository};
use rmcp::{model::CallToolRequestParams, ServiceExt};
use serde_json::{json, Value};
use writes::{scoped, writable_host};

fn tool(operation: &str, call: &Value) -> CallToolRequestParams {
    CallToolRequestParams::new(operation.to_owned()).with_arguments(
        json!({"protocol_version":9,"call":call})
            .as_object()
            .unwrap()
            .clone(),
    )
}

fn save_request(etag: &str, request_id: &str, description: &str) -> Value {
    json!({
        "request_id": request_id,
        "actor": "worker",
        "expected_etag": etag,
        "update": {
            "scope_id": "default",
            "id": "req_shared",
            "description": description
        },
        "relationships": null
    })
}

#[tokio::test]
async fn requirement_save_has_native_parity_bounded_history_and_an_authoritative_receipt() {
    let repo = Repository::new("The shared graph is readable.");
    let store = StateStore::new(repo.layout.clone());
    let scope = ScopeId::new("default").unwrap();
    let requirement = StableId::new("req_shared").unwrap();
    let request = save_request(
        &store
            .requirement_edit_state(&scope, &requirement)
            .unwrap()
            .etag,
        "request_one",
        "First review change",
    );
    let native = store
        .save_requirement(serde_json::from_value::<SaveRequirement>(request.clone()).unwrap())
        .unwrap();
    let host = writable_host(&repo);

    let (status, transported) = call(&host, "save-requirement", scoped(&request)).await;
    assert_eq!(status, 200, "{transported}");
    assert_eq!(transported, serde_json::to_value(&native).unwrap());

    let receipt = json!({
        "requirement_id": "req_shared",
        "request_id": "request_one",
        "actor": "worker",
        "declared_by": null
    });
    let (status, found) = call(&host, "requirement-save-receipt", scoped(&receipt)).await;
    assert_eq!(status, 200, "{found}");
    assert_eq!(found, transported);

    let state = store.requirement_edit_state(&scope, &requirement).unwrap();
    let second = save_request(&state.etag, "request_two", "Second review change");
    let (status, second_result) = call(&host, "save-requirement", scoped(&second)).await;
    assert_eq!(status, 200, "{second_result}");

    let history = json!({"requirement_id":"req_shared","limit":1,"cursor":null});
    let (status, first_page) = call(&host, "review-history", scoped(&history)).await;
    assert_eq!(status, 200, "{first_page}");
    assert_eq!(first_page["entries"].as_array().unwrap().len(), 1);
    let cursor = first_page["next_cursor"].as_str().unwrap();
    let history = json!({"requirement_id":"req_shared","limit":1,"cursor":cursor});
    let (status, last_page) = call(&host, "review-history", scoped(&history)).await;
    assert_eq!(status, 200, "{last_page}");
    assert_eq!(last_page["entries"].as_array().unwrap().len(), 1);
    assert!(last_page["next_cursor"].is_null());
    host.shutdown().await;
}

#[tokio::test]
async fn scope_mismatches_are_typed_and_denied_mutations_change_nothing() {
    let repo = Repository::new("The shared graph is readable.");
    let store = StateStore::new(repo.layout.clone());
    let scope = ScopeId::new("default").unwrap();
    let requirement = StableId::new("req_shared").unwrap();
    let mut request = save_request(
        &store
            .requirement_edit_state(&scope, &requirement)
            .unwrap()
            .etag,
        "request_mismatch",
        "Refused change",
    );
    request["update"]["scope_id"] = json!("other");
    let writable = writable_host(&repo);
    let (status, refusal) = call(&writable, "save-requirement", scoped(&request)).await;
    assert_eq!(status, 400, "{refusal}");
    assert_eq!(refusal["error"]["kind"], "scope_mismatch");
    writable.shutdown().await;

    let before = repo.bytes();
    let denied = host(&[("selected", &repo)], &["selected"]);
    let (status, refusal) = call(&denied, "save-requirement", scoped(&request)).await;
    assert_eq!(status, 403, "{refusal}");
    assert_eq!(refusal["error"]["kind"], "access_denied");
    assert_eq!(repo.bytes(), before);
    denied.shutdown().await;
}

#[tokio::test]
async fn writable_mcp_advertises_and_invokes_requirement_review_operations() {
    let repo = Repository::new("The shared graph is readable.");
    let store = StateStore::new(repo.layout.clone());
    let scope = ScopeId::new("default").unwrap();
    let requirement = StableId::new("req_shared").unwrap();
    let request = save_request(
        &store
            .requirement_edit_state(&scope, &requirement)
            .unwrap()
            .etag,
        "request_mcp",
        "The MCP transport saves this change.",
    );
    let host = writable_host(&repo);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server_host = host.clone();
    let server = tokio::spawn(async move { server_host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let definitions = client.list_all_tools().await.unwrap();
    for operation in [
        "requirement-edit-state",
        "save-requirement",
        "requirement-save-receipt",
        "review-history",
        "review-evidence",
        "create-review-requirement",
        "requirement-creation-receipt",
        "write-discussion",
        "discussion-receipt",
        "review-discussions",
        "review-discussion-messages",
        "submit-requirement-review",
        "decide-requirement-review",
        "withdraw-requirement-review",
        "requirement-decision-state",
        "requirement-review-receipt",
    ] {
        assert!(
            definitions
                .iter()
                .any(|definition| definition.name == operation),
            "missing MCP operation {operation}"
        );
    }
    let result = client
        .call_tool(tool("save-requirement", &scoped(&request)))
        .await
        .unwrap();
    assert_ne!(result.is_error, Some(true));
    assert_eq!(result.structured_content.unwrap()["outcome"], "enrolled");

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
    host.shutdown().await;
}
