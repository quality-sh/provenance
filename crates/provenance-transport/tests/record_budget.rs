//! The HTTP create and update of a Rule whose stored row passes the record
//! budget answer 400 `record_too_large` with the record, its size and the
//! budget, and the Rule and its evidence read stay as they were.
#![cfg(feature = "test-fixture")]
use crate::support;
use provenance_core::{NodeType, ScopeId, StableId};
use provenance_macros::verifies;
use serde_json::json;
use support::records::Repository;
use support::resource_http::{call, call_with_headers, host};

/// The bytes of one stored row of a record or a binding. The store owns the
/// number in `read_budget::RECORD_BYTES`.
const BUDGET: u64 = 65_536;

#[tokio::test]
#[verifies("rule_writer_refuses_a_row_over_the_record_budget", examples)]
async fn a_rule_create_and_update_over_the_budget_answer_record_too_large() {
    let repo = Repository::new("The evidence is readable.");
    let host = host(&repo, true);
    let description = "x".repeat(72_900);
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
    let (_, before) = call(&host, "GET", "/rules/rule_shared", None).await;
    let create = json!({"data": {
        "id": "rule_long", "requirement_ids": ["req_shared"], "resolution_ids": [],
        "statement": "The evidence is readable.", "status": "active", "severity": "high",
        "description": description
    }});
    let update = json!({"data": {"description": description}});

    let (created_status, created) = call(&host, "POST", "/rules", Some(create)).await;
    let (updated_status, updated) = call_with_headers(
        &host,
        "PATCH",
        "/rules/rule_shared",
        Some(update),
        &[("if-match", &etag)],
    )
    .await;
    let (_, after) = call(&host, "GET", "/rules/rule_shared", None).await;
    let (missing, _) = call(&host, "GET", "/rules/rule_long", None).await;
    let (evidence, _) = call(&host, "GET", "/rules/rule_shared/evidence", None).await;

    for (status, refusal, id) in [
        (created_status, &created, "rule_long"),
        (updated_status, &updated, "rule_shared"),
    ] {
        let error = &refusal["error"];
        assert_eq!(status, 400, "{refusal}");
        assert_eq!(
            (&error["kind"], &error["record_id"], &error["limit"]),
            (&json!("record_too_large"), &json!(id), &json!(BUDGET)),
            "{refusal}"
        );
        assert!(error["bytes"].as_u64().unwrap() > 72_900, "{refusal}");
    }
    assert_eq!(after["data"], before["data"]);
    assert_eq!((missing, evidence), (404, 200));
}
