use super::super::common::model::{enum_names, PROPOSAL_TYPES};
use serde_json::{json, Value};

pub(in crate::porcelain::schema) fn schema() -> Value {
    json!({
        "title": "ProposalCard",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "scope_id",
            "id",
            "proposal_key",
            "proposal_type",
            "title",
            "summary",
            "traceability",
            "promotion_state"
        ],
        "properties": {
            "scope_id": {"$ref": "#/$defs/scopeId"},
            "id": {"$ref": "#/$defs/stableId"},
            "proposal_key": {"type": "string", "minLength": 1},
            "proposal_type": {"enum": enum_names(&PROPOSAL_TYPES)},
            "title": {"type": "string"},
            "summary": {"type": "string"},
            "confidence": {"type": "number", "minimum": 0.0, "maximum": 1.0},
            "traceability": {"$ref": "#/$defs/proposalTraceability"},
            "promotion_state": {"const": "proposed"},
            "builds_on": {"type": "array", "uniqueItems": true, "items": {"$ref": "#/$defs/stableId"}}
        }
    })
}
