//! The api tool refuses a Rule create and a Rule update whose stored row
//! passes the record budget, with the record, its size and the budget, and
//! the Rule stays as it was.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_core::{NodeType, ScopeId, StableId};
use provenance_macros::verifies;
use serde_json::json;
use support::api_session::ApiSession;
use support::records::Repository;
use support::resource_http::host;

/// The bytes of one stored row of a record or a binding. The store owns the
/// number in `read_budget::RECORD_BYTES`.
const BUDGET: u64 = 65_536;

#[tokio::test]
#[verifies("rule_writer_refuses_a_row_over_the_record_budget", examples)]
async fn the_api_tool_refuses_a_rule_create_and_update_over_the_budget() {
    let repo = Repository::new("The evidence is readable.");
    let etag = repo
        .open()
        .store()
        .record_edit_state(
            &ScopeId::new("default").unwrap(),
            NodeType::Rule,
            &StableId::new("rule_shared").unwrap(),
        )
        .unwrap()
        .etag;
    let session = ApiSession::start(host(&repo, true)).await;
    let description = "x".repeat(72_900);
    let before = session.call(json!({"path": "rules/rule_shared"})).await;

    let created = session
        .call(json!({
            "path": "rules",
            "method": "post",
            "input": {
                "id": "rule_long", "requirement_ids": ["req_shared"], "resolution_ids": [],
                "statement": "The evidence is readable.", "status": "active",
                "severity": "high", "description": description
            }
        }))
        .await;
    let updated = session
        .call(json!({
            "path": "rules/rule_shared",
            "method": "patch",
            "header": {"If-Match": etag},
            "input": {"description": description}
        }))
        .await;
    let after = session.call(json!({"path": "rules/rule_shared"})).await;

    for (result, id, bytes) in [
        (&created, "rule_long", 73_022),
        (&updated, "rule_shared", 73_021),
    ] {
        assert_eq!(result.is_error, Some(true), "{result:?}");
        let error = &result.structured_content.as_ref().unwrap()["error"];
        assert_eq!(error, &too_large(id, bytes), "{error}");
    }
    assert_eq!(
        after.structured_content.unwrap()["data"],
        before.structured_content.unwrap()["data"]
    );
    session.shutdown().await;
}

/// The refusal of the Rule `id` whose row uses `bytes` bytes, most of them
/// in its description.
fn too_large(id: &str, bytes: u64) -> serde_json::Value {
    json!({
        "kind": "record_too_large",
        "record_id": id,
        "field": "description",
        "bytes": bytes,
        "limit": BUDGET,
        "message": format!(
            "record `{id}` uses {bytes} bytes and the limit is {BUDGET} bytes; \
             make the field `description` shorter"
        )
    })
}
