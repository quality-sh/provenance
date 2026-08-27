//! The JSON Schema for the repository manifest, including the closed rbac
//! section.

use serde_json::{json, Value};

pub(in crate::handlers::schema) fn schema() -> Value {
    json!({
        "title": "Manifest",
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "scopes"],
        "properties": {
            "schema_version": {"const": 1},
            "scopes": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["id", "path_prefix"],
                    "properties": {
                        "id": {"type": "string", "pattern": "^[a-z0-9_-]+$"},
                        "path_prefix": {"type": "string"}
                    }
                }
            },
            "disposition_actor_ids": {
                "type": "array",
                "items": {"type": "string"}
            },
            "rbac": {
                "type": "object",
                "additionalProperties": false,
                "required": ["assignments"],
                "properties": {
                    "assignments": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": ["actor_id", "capabilities", "scopes"],
                            "properties": {
                                "actor_id": {"type": "string", "pattern": "\\S"},
                                "identity_type": {
                                    "enum": ["human", "agent", "service"]
                                },
                                "capabilities": {
                                    "type": "array",
                                    "items": {
                                        "enum": ["read", "edit", "execute", "manifest-write"]
                                    }
                                },
                                "scopes": {
                                    "type": "array",
                                    "items": {"type": "string"}
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}
