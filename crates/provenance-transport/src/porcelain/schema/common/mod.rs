use serde_json::{json, Value};

pub(super) mod model;

use model::{
    enum_names, ARTIFACT_CHANGE_TYPES, EVIDENCE_QUALITIES, IDEATION_EVIDENCE_TYPES,
    IDEATION_TARGET_TYPES, PROPOSAL_TYPES, SPECULATION_MARKERS, UNCERTAINTY_LEVELS,
};

#[allow(clippy::too_many_lines)]
pub(super) fn definitions() -> Value {
    json!({
        "stableId": {
            "type": "string",
            "pattern": "^[a-z0-9_-]+$",
            "description": "StableId: lowercase ASCII letters, digits, '_' or '-'."
        },
        "scopeId": {
            "type": "string",
            "pattern": "^[a-z0-9_-]+$",
            "description": "ScopeId: lowercase ASCII letters, digits, '_' or '-'."
        },
        "stableIdArray": {
            "type": "array",
            "items": {"$ref": "#/$defs/stableId"}
        },
        "ideationTarget": {
            "type": "object",
            "additionalProperties": false,
            "required": ["artifact_type", "artifact_id"],
            "properties": {
                "artifact_type": {"enum": enum_names(&IDEATION_TARGET_TYPES)},
                "artifact_id": {"$ref": "#/$defs/stableId"}
            }
        },
        "evidenceReference": {
            "type": "object",
            "additionalProperties": false,
            "required": ["reference_id", "evidence_type", "summary"],
            "properties": {
                "reference_id": {"$ref": "#/$defs/stableId"},
                "evidence_type": {"enum": enum_names(&IDEATION_EVIDENCE_TYPES)},
                "summary": {"type": "string"},
                "file_path": {"type": "string"},
                "line": {"type": "integer", "minimum": 1}
            }
        },
        "materialClaim": {
            "type": "object",
            "additionalProperties": false,
            "required": ["claim_id", "statement", "evidence_type", "evidence_reference_ids"],
            "properties": {
                "claim_id": {"$ref": "#/$defs/stableId"},
                "statement": {"type": "string"},
                "evidence_type": {"enum": enum_names(&IDEATION_EVIDENCE_TYPES)},
                "evidence_reference_ids": {"$ref": "#/$defs/stableIdArray"},
                "confidence": {"type": "number", "minimum": 0.0, "maximum": 1.0}
            }
        },
        "claimChallenge": {
            "type": "object",
            "additionalProperties": false,
            "required": ["claim_id", "objection"],
            "properties": {
                "claim_id": {"$ref": "#/$defs/stableId"},
                "objection": {"type": "string"}
            }
        },
        "suggestedArtifactChange": {
            "type": "object",
            "additionalProperties": false,
            "required": ["artifact_type", "change_type", "supporting_claim_ids", "summary"],
            "properties": {
                "artifact_type": {"enum": enum_names(&IDEATION_TARGET_TYPES)},
                "artifact_id": {"$ref": "#/$defs/stableId"},
                "change_type": {"enum": enum_names(&ARTIFACT_CHANGE_TYPES)},
                "supporting_claim_ids": {"$ref": "#/$defs/stableIdArray"},
                "summary": {"type": "string"}
            }
        },
        "unsupportedRecommendation": {
            "type": "object",
            "additionalProperties": false,
            "required": ["recommendation", "marker"],
            "properties": {
                "recommendation": {"type": "string"},
                "marker": {"enum": enum_names(&SPECULATION_MARKERS)}
            }
        },
        "uncertainty": {
            "type": "object",
            "additionalProperties": false,
            "required": ["level", "rationale"],
            "properties": {
                "level": {"enum": enum_names(&UNCERTAINTY_LEVELS)},
                "rationale": {"type": "string"}
            }
        },
        "consensusFinding": {
            "type": "object",
            "additionalProperties": false,
            "required": ["statement", "supporting_participant_slots", "evidence_reference_ids"],
            "properties": {
                "statement": {"type": "string"},
                "supporting_participant_slots": {"type": "array", "items": {"type": "string"}},
                "evidence_reference_ids": {"$ref": "#/$defs/stableIdArray"}
            }
        },
        "contestedClaim": {
            "type": "object",
            "additionalProperties": false,
            "required": ["claim_id", "statement", "supporting_participant_slots", "opposing_participant_slots", "evidence_quality"],
            "properties": {
                "claim_id": {"$ref": "#/$defs/stableId"},
                "statement": {"type": "string"},
                "supporting_participant_slots": {"type": "array", "items": {"type": "string"}},
                "opposing_participant_slots": {"type": "array", "items": {"type": "string"}},
                "evidence_quality": {"enum": enum_names(&EVIDENCE_QUALITIES)}
            }
        },
        "minorityObjection": {
            "type": "object",
            "additionalProperties": false,
            "required": ["participant_slot", "objection", "evidence_reference_ids"],
            "properties": {
                "participant_slot": {"type": "string"},
                "objection": {"type": "string"},
                "evidence_reference_ids": {"$ref": "#/$defs/stableIdArray"}
            }
        },
        "evidenceGap": {
            "type": "object",
            "additionalProperties": false,
            "required": ["question", "needed_evidence_type", "blocking_promotion"],
            "properties": {
                "question": {"type": "string"},
                "needed_evidence_type": {"enum": enum_names(&IDEATION_EVIDENCE_TYPES)},
                "blocking_promotion": {"type": "boolean"}
            }
        },
        "unsupportedSpeculation": {
            "type": "object",
            "additionalProperties": false,
            "required": ["statement", "originating_participant_slots", "marker"],
            "properties": {
                "statement": {"type": "string"},
                "originating_participant_slots": {"type": "array", "items": {"type": "string"}},
                "marker": {"enum": enum_names(&SPECULATION_MARKERS)}
            }
        },
        "suggestedArtifact": {
            "type": "object",
            "additionalProperties": false,
            "required": ["proposal_key", "proposal_type", "summary", "origin_participant_slots"],
            "properties": {
                "proposal_id": {"$ref": "#/$defs/stableId"},
                "proposal_key": {"type": "string", "minLength": 1},
                "proposal_type": {"enum": enum_names(&PROPOSAL_TYPES)},
                "summary": {"type": "string"},
                "origin_participant_slots": {"type": "array", "items": {"type": "string"}}
            }
        },
        "requiredHumanDecision": {
            "type": "object",
            "additionalProperties": false,
            "required": ["decision_key", "prompt", "blocks_promotion"],
            "properties": {
                "decision_key": {"$ref": "#/$defs/stableId"},
                "prompt": {"type": "string"},
                "blocks_promotion": {"type": "boolean"}
            }
        },
        "proposalTraceability": {
            "type": "object",
            "additionalProperties": false,
            "required": ["target", "source_ids", "evidence_references", "supporting_claim_ids"],
            "properties": {
                "target": {"$ref": "#/$defs/ideationTarget"},
                "source_ids": {"$ref": "#/$defs/stableIdArray"},
                "evidence_references": {"type": "array", "items": {"$ref": "#/$defs/evidenceReference"}},
                "supporting_claim_ids": {"$ref": "#/$defs/stableIdArray"}
            }
        }
    })
}
