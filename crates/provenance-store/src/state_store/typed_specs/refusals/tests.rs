//! The order of the deletions that review history keeps. This test is a
//! characterization of `refused_deletions` and has no Rule binding: no Rule
//! makes the order an obligation. It shows which refusal the apply gives
//! when a document has more than one.

use super::*;
use crate::state_store::typed_specs::view::ReviewHistory;
use serde_json::json;

fn id(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn record<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).unwrap()
}

fn requirement(requirement: &str) -> provenance_core::Requirement {
    record(
        json!({"scope_id": "default", "id": requirement, "statement": "The host stops.",
                  "status": "active"}),
    )
}

fn topic(topic: &str, requirement: &str) -> provenance_core::Topic {
    record(
        json!({"scope_id": "default", "id": topic, "requirement_id": requirement,
                  "title": "Test", "status": "open", "links": []}),
    )
}

fn submission_refusal(kind: &str, record: &str, requirement: &str) -> DeletionRefusal {
    DeletionRefusal::ReviewSubmission {
        message: format!(
            "cannot delete {kind} {record} because it has a review submission; \
             keep requirement {requirement} in the typed document"
        ),
    }
}

#[test]
fn a_disposition_comes_first_then_submissions_in_review_record_order() {
    let view = TypedSpecView {
        requirements: vec![requirement("req_a"), requirement("req_b")],
        topics: vec![topic("topic_x", "req_a"), topic("topic_y", "req_b")],
        review: ReviewHistory {
            submitted: vec![
                (NodeType::Topic, id("topic_y")),
                (NodeType::Requirement, id("req_b")),
                (NodeType::Topic, id("topic_x")),
                (NodeType::Requirement, id("req_a")),
            ],
            dispositions: vec![record(json!({
                "scope_id": "default", "id": "disposition_x", "proposal_id": "proposal_x",
                "decision": "rejected", "rationale": "Reviewed",
                "actor": {"identity_type": "human", "id": "reviewer"},
                "canonical_artifact": {"artifact_type": "topic", "artifact_id": "topic_x"}
            }))],
        },
        ..TypedSpecView::default()
    };
    let survivors = ScopeRecords {
        sources: &[],
        requirements: &[],
        rules: &[],
        resolutions: &[],
        topics: &[],
        questions: &[],
    };
    let cascade = [
        CascadeDeletion {
            kind: NodeType::Topic,
            id: id("topic_x"),
            requirements: vec![id("req_a")],
        },
        CascadeDeletion {
            kind: NodeType::Topic,
            id: id("topic_y"),
            requirements: vec![id("req_b")],
        },
    ];

    let refused = refused_deletions(&view, &survivors, &cascade);

    let order = refused
        .iter()
        .map(|entry| (entry.kind, entry.id.as_str(), &entry.refusal))
        .collect::<Vec<_>>();
    let disposition = DeletionRefusal::Disposition {
        disposition_id: id("disposition_x"),
        message: "cannot delete canonical topic topic_x because disposition disposition_x \
                  names it; keep requirement req_a in the typed document"
            .to_owned(),
    };
    assert_eq!(
        order,
        [
            (NodeType::Topic, "topic_x", &disposition),
            (
                NodeType::Requirement,
                "req_a",
                &submission_refusal("requirement", "req_a", "req_a")
            ),
            (
                NodeType::Requirement,
                "req_b",
                &submission_refusal("requirement", "req_b", "req_b")
            ),
            (
                NodeType::Topic,
                "topic_y",
                &submission_refusal("topic", "topic_y", "req_b")
            ),
        ]
    );
}
