#![cfg(feature = "test-fixture")]
#[allow(dead_code)]
mod support {
    pub mod mcp;
    #[path = "../../../provenance-transport/tests/support/records.rs"]
    pub mod records;
    #[path = "../../../provenance-transport/tests/support/resource_http.rs"]
    pub mod resource_http;
}

use provenance_macros::verifies;
use serde_json::json;
use support::records::Repository;
use support::resource_http::host;

fn source_etag(repo: &Repository) -> String {
    repo.open()
        .store()
        .record_edit_state(
            &provenance_core::ScopeId::new("default").unwrap(),
            provenance_core::NodeType::Source,
            &provenance_core::StableId::new("source_shared").unwrap(),
        )
        .unwrap()
        .etag
}

#[tokio::test]
#[verifies("rule_review_update_guard_header", examples)]
async fn mcp_source_update_takes_the_guard_as_if_match() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let etag = source_etag(&repo);
    let session = support::mcp::Session::start(host(&repo, true)).await;
    let update =
        |name: &str| json!({"target":"source_shared","if_match":etag,"data":{"name":name}});

    let current = support::mcp::call(&session.client, "update", update("Source B")).await;
    assert_ne!(current.is_error, Some(true), "{current:?}");
    let stale = support::mcp::call(&session.client, "update", update("Source C")).await;
    assert_eq!(stale.is_error, Some(true), "{stale:?}");
    assert!(
        format!("{stale:?}").contains("record_edit_conflict"),
        "{stale:?}"
    );
    session.close().await;
}
