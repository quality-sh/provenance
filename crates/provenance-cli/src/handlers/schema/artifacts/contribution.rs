use super::super::common::model::{enum_names, CONTRIBUTION_STANCES};
use serde_json::{json, Value};

pub(in crate::handlers::schema) fn schema() -> Value {
    json!({
        "title": "Contribution",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "scope_id",
            "id",
            "target",
            "participant_slot",
            "stance",
            "strongest_finding",
            "evidence_references",
            "material_claims",
            "risks",
            "objections",
            "challenges",
            "suggested_artifact_changes",
            "unsupported_recommendations",
            "uncertainty",
            "open_questions"
        ],
        "properties": {
            "scope_id": {"$ref": "#/$defs/scopeId"},
            "id": {"$ref": "#/$defs/stableId"},
            "target": {"$ref": "#/$defs/ideationTarget"},
            "participant_slot": {"type": "string", "minLength": 1},
            "stance": {"enum": enum_names(&CONTRIBUTION_STANCES)},
            "strongest_finding": {"type": "string"},
            "evidence_references": {"type": "array", "items": {"$ref": "#/$defs/evidenceReference"}},
            "material_claims": {"type": "array", "items": {"$ref": "#/$defs/materialClaim"}},
            "risks": {"type": "array", "items": {"type": "string"}},
            "objections": {"type": "array", "items": {"type": "string"}},
            "challenges": {"type": "array", "items": {"$ref": "#/$defs/claimChallenge"}},
            "suggested_artifact_changes": {"type": "array", "items": {"$ref": "#/$defs/suggestedArtifactChange"}},
            "unsupported_recommendations": {"type": "array", "items": {"$ref": "#/$defs/unsupportedRecommendation"}},
            "uncertainty": {"$ref": "#/$defs/uncertainty"},
            "open_questions": {"type": "array", "items": {"type": "string"}}
        }
    })
}
