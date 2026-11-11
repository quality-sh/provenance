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
