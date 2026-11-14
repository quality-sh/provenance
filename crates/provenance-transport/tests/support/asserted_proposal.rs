//! A proposal that a synthesis packet qualifies, with its assertion.

use super::records::Repository;
use super::resource_http::{call, host};
use serde_json::{json, Value};

/// Creates the proposal `proposal_id` about the requirement `req_shared`
/// with its assertion `assertion_a`, and the evidence of the assertion: the
/// contribution `contribution_evidence` with the claim `claim_a` and the
/// synthesis packet `synthesis_evidence` that suggests the proposal.
///
/// The routes create the evidence. No route creates a proposal that a packet
/// qualifies, so one store call creates the proposal and its assertion, as a
/// swarm landing does. That call writes the landings file, and the proposal
/// file and the assertion file do not hold the two records.
pub async fn create_asserted_proposal(repo: &Repository, proposal_id: &str) {
    let writer = host(repo, true);
    for (path, body) in evidence(proposal_id) {
        let (status, created) = call(&writer, "POST", path, Some(body)).await;
        assert_eq!(status, 200, "{path}: {created}");
    }
    let proposal = json!({
        "scope_id":"default", "id":proposal_id, "proposal_key":proposal_id,
        "proposal_type":"requirement_candidate", "title":"Candidate",
        "summary":"Clarify the shared requirement.",
        "traceability":{"target":target(), "source_ids":[], "evidence_references":[],
            "supporting_claim_ids":["claim_a"]},
        "builds_on":[], "promotion_state":"proposed"
    });
    repo.open()
        .store()
        .create_asserted_proposal(
            serde_json::from_value(proposal).unwrap(),
            serde_json::from_value(assertion_row(proposal_id)).unwrap(),
        )
        .unwrap();
}

/// The stored row of the assertion `assertion_a` of the proposal
/// `proposal_id`.
pub fn assertion_row(proposal_id: &str) -> Value {
    json!({"scope_id":"default", "id":"assertion_a", "proposal_id":proposal_id,
        "synthesis_packet_id":"synthesis_evidence", "supporting_claim_ids":["claim_a"]})
}

fn target() -> Value {
    json!({"artifact_type":"requirement","artifact_id":"req_shared"})
}

/// The evidence for an assertion of the proposal `proposal_id`. Each item is
/// the create route and its body.
fn evidence(proposal_id: &str) -> [(&'static str, Value); 2] {
    [
        (
            "/contributions",
            json!({"data":{
                "id":"contribution_evidence", "target":target(),
                "participant_slot":"reviewer", "stance":"support", "strongest_finding":"Observed",
                "evidence_references":[{"reference_id":"evidence_a","evidence_type":"source","summary":"Pinned"}],
                "material_claims":[{"claim_id":"claim_a","statement":"Observed",
                    "evidence_type":"source","evidence_reference_ids":["evidence_a"]}],
                "risks":[], "objections":[], "challenges":[], "suggested_artifact_changes":[],
                "unsupported_recommendations":[], "uncertainty":{"level":"low","rationale":"Direct"},
                "open_questions":[]
            }}),
        ),
        (
            "/synthesis-packets",
            json!({"data":{
                "id":"synthesis_evidence", "target":target(), "summary":"Adjudicated",
                "consensus":[], "contested_claims":[], "minority_objections":[],
                "evidence_gaps":[], "unsupported_speculation":[], "open_questions":[],
                "suggested_artifacts":[{"proposal_id":proposal_id,"proposal_key":proposal_id,
                    "proposal_type":"requirement_candidate","summary":"Candidate",
                    "origin_participant_slots":["reviewer"]}],
                "required_human_decisions":[]
            }}),
        ),
    ]
}
