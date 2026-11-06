use serde_json::{json, Value};

pub(in crate::handlers::schema) fn schema() -> Value {
    json!({
        "title": "SynthesisPacket",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "scope_id",
            "id",
            "target",
            "summary",
            "consensus",
            "contested_claims",
            "minority_objections",
            "evidence_gaps",
            "unsupported_speculation",
            "open_questions",
            "suggested_artifacts",
            "required_human_decisions"
        ],
        "properties": {
            "scope_id": {"$ref": "#/$defs/scopeId"},
            "id": {"$ref": "#/$defs/stableId"},
            "target": {"$ref": "#/$defs/ideationTarget"},
            "summary": {"type": "string"},
            "consensus": {"type": "array", "items": {"$ref": "#/$defs/consensusFinding"}},
            "contested_claims": {"type": "array", "items": {"$ref": "#/$defs/contestedClaim"}},
            "minority_objections": {"type": "array", "items": {"$ref": "#/$defs/minorityObjection"}},
            "evidence_gaps": {"type": "array", "items": {"$ref": "#/$defs/evidenceGap"}},
            "unsupported_speculation": {"type": "array", "items": {"$ref": "#/$defs/unsupportedSpeculation"}},
            "open_questions": {"type": "array", "items": {"type": "string"}},
            "suggested_artifacts": {"type": "array", "items": {"$ref": "#/$defs/suggestedArtifact"}},
            "required_human_decisions": {"type": "array", "items": {"$ref": "#/$defs/requiredHumanDecision"}}
        }
    })
}
