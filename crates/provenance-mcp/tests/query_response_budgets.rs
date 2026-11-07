#![cfg(feature = "test-fixture")]

use provenance_core::protocol::QUERY_RESPONSE_BYTES;
use provenance_core::{RequirementStatus, ScopeId, StableId};
use provenance_store::state_store::CreateRequirementInput;
use provenance_transport::StatementHost;
use serde_json::json;

#[allow(dead_code)]
mod support {
    pub mod mcp;
    #[path = "../../../provenance-transport/tests/support/records.rs"]
    pub mod records;
}
use provenance_macros::verifies;
use support::{
    mcp::{call as call_tool, Session},
    records::Repository,
};

fn host(repo: &Repository) -> StatementHost {
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
    StatementHost::with_fixture_access(access)
}

fn add_child(repo: &Repository, index: usize, description_bytes: usize) {
    let store = repo.open().store();
    let scope = ScopeId::new("default").unwrap();
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope,
            id: StableId::new(format!("req_large_{index:03}")).unwrap(),
            statement: "The graph includes this record.".into(),
            description: Some("x".repeat(description_bytes)),
            status: RequirementStatus::Active,
            refines: vec![StableId::new("req_shared").unwrap()],
            depends_on: vec![],
            supersedes: vec![],
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
}

fn add_large_children(repo: &Repository, count: usize) {
    for index in 0..count {
        add_child(repo, index, 60_000);
    }
}

/// The MCP get and search tools are typed callers, so the store bounds their pages before
/// the answer reaches the tool: a traversal over the budget refuses, a search page stops
/// at the budget and continues through its cursor.
#[verifies("rule_query_pages_bound_shared_reads", examples)]
#[tokio::test]
async fn mcp_get_and_search_keep_the_store_page_bound() {
    let repo = Repository::new("The shared rule is readable.");
    add_large_children(&repo, 20);
    let session = Session::start(host(&repo)).await;
    let client = &session.client;

    let get = call_tool(
        client,
        "get",
        json!({"target":"req_shared", "view":"children"}),
    )
    .await;
    assert_eq!(get.is_error, Some(true), "{get:?}");
    let error = &get.structured_content.as_ref().unwrap()["error"];
    assert_eq!(error["kind"], "operation_failed", "{error}");
    assert_eq!(error["message"], "page read work exceeds the engine budget");

    let search = call_tool(client, "search", json!({"text":"record", "limit":200})).await;
    assert_ne!(search.is_error, Some(true), "{search:?}");
    let page = search.structured_content.as_ref().unwrap();
    let returned = page["nodes"].as_array().unwrap().len();
    assert!(
        returned < 20,
        "the search page stops before the budget: {returned}"
    );
    assert_eq!(page["has_more"], true);
    assert!(page["next_cursor"].is_string());
    assert!(serde_json::to_vec(page).unwrap().len() <= QUERY_RESPONSE_BYTES);

    session.close().await;
}
