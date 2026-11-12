use serde_json::{json, Value};

/// A contribution with no claims about the requirement `req_shared`.
#[allow(dead_code)]
pub fn contribution(id: &str) -> Value {
    json!({
        "id":id,
        "target":{"artifact_type":"requirement","artifact_id":"req_shared"},
        "participant_slot":"reviewer", "stance":"support",
        "strongest_finding":"Original finding", "evidence_references":[],
        "material_claims":[], "risks":[], "objections":[], "challenges":[],
        "suggested_artifact_changes":[], "unsupported_recommendations":[],
        "uncertainty":{"level":"low","rationale":"Direct evidence"},
        "open_questions":[]
    })
}

/// A synthesis packet with no suggestions about the requirement `req_shared`.
#[allow(dead_code)]
pub fn synthesis(id: &str) -> Value {
    json!({
        "id":id,
        "target":{"artifact_type":"requirement","artifact_id":"req_shared"},
        "summary":"Original summary", "consensus":[], "contested_claims":[],
        "minority_objections":[], "evidence_gaps":[], "unsupported_speculation":[],
        "open_questions":[], "suggested_artifacts":[], "required_human_decisions":[]
    })
}

/// The evidence for an assertion of the proposal `proposal_id`: a
/// contribution with the claim `claim_a` and a synthesis packet that
/// suggests the proposal. Each item is the create route and its body.
#[allow(dead_code)]
pub fn evidence(proposal_id: &str) -> [(&'static str, Value); 2] {
    let target = json!({"artifact_type":"requirement","artifact_id":"req_shared"});
    [
        (
            "/contributions",
            json!({"data":{
                "id":"contribution_evidence", "target":target,
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
                "id":"synthesis_evidence", "target":target, "summary":"Adjudicated",
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
