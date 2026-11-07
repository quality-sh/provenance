//! A shaping claim has one holder; another actor cannot take or drop it.
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
use support::mcp::Session;
use support::records::Repository;
use support::resource_http::host;

#[tokio::test]
#[verifies("rule_invalid_update_refusal_gives_reason", examples)]
async fn an_mcp_release_of_an_unclaimed_topic_gives_the_reason() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let session = Session::start(host(&repo, true)).await;
    let refused = support::mcp::call(
        &session.client,
        "change",
        json!({"action":"release","target":"topic_shared","actor":"alice"}),
    )
    .await;
    assert_eq!(refused.is_error, Some(true), "{refused:?}");
    assert_eq!(
        refused.structured_content.as_ref().unwrap()["error"],
        json!({"kind":"invalid_update", "message":"topic topic_shared is not claimed"}),
        "{refused:?}"
    );
    session.close().await;
}
