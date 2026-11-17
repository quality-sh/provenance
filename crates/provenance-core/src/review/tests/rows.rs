//! Stored review rows for the tests of the reading.

use crate::review::{ReviewRowFamily, RowAt};
use crate::{DispositionRecord, ProposalCard, StableId, Withdrawal};
use serde_json::json;

pub(super) const REQ_A: &str = "req_a";

pub(super) fn id(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

pub(super) fn submission(id: &str, record: &str, cycle: u64) -> ProposalCard {
    serde_json::from_value(json!({
        "scope_id": "default", "id": id,
        "proposal_key": format!("review:requirement:{record}:{cycle}"),
        "proposal_type": "record_revision", "title": "Review", "summary": "Review",
        "traceability": {
            "target": {"artifact_type": "requirement", "artifact_id": record},
            "source_ids": [], "evidence_references": [], "supporting_claim_ids": []
        },
        "promotion_state": "proposed",
        "record_revision": {"revision": "rev_a"}
    }))
    .unwrap()
}

/// A line of the submission `id` whose summary a person changed.
pub(super) fn changed_submission(id: &str, record: &str, cycle: u64) -> ProposalCard {
    let mut changed = submission(id, record, cycle);
    changed.summary = "Changed by hand".to_owned();
    changed
}

pub(super) fn decision(id: &str, submission: &str, result: &str, actor: &str) -> DispositionRecord {
    serde_json::from_value(json!({
        "scope_id": "default", "id": id, "proposal_id": submission, "decision": result,
        "actor": {"identity_type": "human", "id": actor}
    }))
    .unwrap()
}

pub(super) fn withdrawal(id: &str, submission: &str, actor: &str) -> Withdrawal {
    serde_json::from_value(json!({
        "scope_id": "default", "id": id, "proposal_id": submission,
        "actor": {"identity_type": "agent", "id": actor}, "reason": null
    }))
    .unwrap()
}

pub(super) const fn at(family: ReviewRowFamily, position: usize) -> RowAt {
    RowAt { family, position }
}

pub(super) const fn submission_at(position: usize) -> RowAt {
    at(ReviewRowFamily::Submission, position)
}

pub(super) const fn decision_at(position: usize) -> RowAt {
    at(ReviewRowFamily::Decision, position)
}

pub(super) const fn withdrawal_at(position: usize) -> RowAt {
    at(ReviewRowFamily::Withdrawal, position)
}
