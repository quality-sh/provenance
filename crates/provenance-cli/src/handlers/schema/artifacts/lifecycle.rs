use serde_json::{json, Value};

pub(in crate::handlers::schema) fn assertion_schema() -> Value {
    json!({
        "title": "Assertion",
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "scope_id", "id", "proposal_id", "synthesis_packet_id", "supporting_claim_ids"],
        "properties": {
            "schema_version": {"const": 1},
            "scope_id": {"$ref": "#/$defs/scopeId"},
            "id": {"$ref": "#/$defs/stableId"},
            "proposal_id": {"$ref": "#/$defs/stableId"},
            "synthesis_packet_id": {"$ref": "#/$defs/stableId"},
            "supporting_claim_ids": {"type": "array", "minItems": 1, "uniqueItems": true, "items": {"$ref": "#/$defs/stableId"}}
        }
    })
}

pub(in crate::handlers::schema) fn disposition_schema() -> Value {
    json!({
        "title": "Disposition",
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "scope_id", "id", "proposal_id", "decision", "rationale", "actor"],
        "properties": {
            "schema_version": {"const": 1},
            "scope_id": {"$ref": "#/$defs/scopeId"},
            "id": {"$ref": "#/$defs/stableId"},
            "proposal_id": {"$ref": "#/$defs/stableId"},
            "decision": {"enum": ["accepted", "rejected", "deferred"]},
            "rationale": {"type": "string", "pattern": ".*\\S.*"},
            "actor": {
                "type": "object", "additionalProperties": false,
                "required": ["identity_type", "id"],
                "properties": {
                    "identity_type": {"enum": ["human", "agent", "service"]},
                    "id": {"type": "string", "pattern": ".*\\S.*"},
                    "name": {"type": "string"}
                }
            },
            "canonical_artifact": {
                "type": "object",
                "additionalProperties": false,
                "required": ["artifact_type", "artifact_id"],
                "properties": {
                    "artifact_type": {"enum": ["source", "requirement", "resolution", "rule", "domain", "boundary", "topic", "question"]},
                    "artifact_id": {"$ref": "#/$defs/stableId"}
                }
            },
            "external_action": {
                "type": "object",
                "additionalProperties": false,
                "required": ["system", "scope", "kind", "key"],
                "properties": {
                    "system": {"type": "string", "pattern": ".*\\S.*"},
                    "scope": {"type": "string", "pattern": ".*\\S.*"},
                    "kind": {"type": "string", "pattern": ".*\\S.*"},
                    "key": {"type": "string", "pattern": ".*\\S.*"}
                }
            }
        }
    })
}
