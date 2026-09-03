use serde_json::{json, Value};

pub(in crate::handlers::schema) fn reference_schema() -> Value {
    json!({
        "title": "GraphReference",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "schema_version", "reference_id", "repository_id", "store_path",
            "scope_id", "commit", "graph_digest"
        ],
        "properties": {
            "schema_version": schema_version(),
            "reference_id": {"type": "string", "pattern": "^grf1_[0-9a-f]{64}$"},
            "repository_id": {"type": "string", "pattern": "^git1_[0-9a-f]{64}$"},
            "store_path": {"const": ".provenance/state"},
            "scope_id": {"type": "string", "pattern": "^[a-z0-9_-]+$"},
            "commit": {"type": "string", "pattern": "^([0-9a-f]{40}|[0-9a-f]{64})$"},
            "graph_digest": {"type": "string", "pattern": "^sha256:[0-9a-f]{64}$"},
            "correlation": {
                "type": "object",
                "additionalProperties": false,
                "required": ["system", "key"],
                "properties": {
                    "system": {"type": "string", "pattern": "\\S"},
                    "key": {"type": "string", "pattern": "\\S"}
                }
            }
        }
    })
}

pub(in crate::handlers::schema) fn export_schema() -> Value {
    json!({
        "title": "GraphReferenceExactExport",
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "operation", "reference_id", "graph_digest", "graph"],
        "properties": {
            "schema_version": schema_version(),
            "operation": {"const": "exact-export"},
            "reference_id": {"type": "string", "pattern": "^grf1_[0-9a-f]{64}$"},
            "graph_digest": {"type": "string", "pattern": "^sha256:[0-9a-f]{64}$"},
            "graph": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "schema_version", "scope", "sources", "domains", "requirements",
                    "boundaries", "topics", "questions", "resolutions", "rules", "edges"
                ],
                "properties": {
                    "schema_version": schema_version(),
                    "scope": {"$ref": "#/$defs/scope"},
                    "sources": record_array("source"),
                    "domains": record_array("domain"),
                    "requirements": record_array("requirement"),
                    "boundaries": record_array("boundary"),
                    "topics": record_array("topic"),
                    "questions": record_array("question"),
                    "resolutions": record_array("resolution"),
                    "rules": record_array("rule"),
                    "verification_bindings": record_array("verificationBinding"),
                    "implementation_bindings": record_array("implementationBinding"),
                    "edges": record_array("edge")
                }
            }
        },
        "$defs": export_definitions()
    })
}

/// The version every published record and envelope must carry.
///
/// `const` states the value and `type` states that it is an integer, because
/// the runtime reads the field into a `u32`: a document writing `1.0` is
/// refused there. JSON Schema drafts 6 and later compare numbers by value and
/// count `1.0` as an integer, so a validator on those drafts still accepts it
/// and the runtime is the stricter of the two; the divergence is pinned by
/// `schema_version_float_is_where_the_replica_is_looser_than_the_runtime` in
/// `crates/provenance-cli/tests/cli_graph_references/schema.rs`. The `type` is
/// worth stating anyway: it says what the field is, and a stricter validator
/// refuses `1.0` on it.
fn schema_version() -> Value {
    json!({"type": "integer", "const": 1})
}

fn record_array(name: &str) -> Value {
    json!({"type": "array", "items": {"$ref": format!("#/$defs/{name}")}})
}

fn declaration_address() -> Value {
    json!({
        "type": "array",
        "minItems": 1,
        "items": {"type": "string", "minLength": 1}
    })
}

#[allow(clippy::needless_pass_by_value)]
fn closed_record(required: &[&str], properties: Value) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties
    })
}

#[allow(clippy::redundant_clone, clippy::too_many_lines)]
fn export_definitions() -> Value {
    let id = json!({"type": "string", "pattern": "^[a-z0-9_-]+$"});
    let id_list = json!({"type": "array", "items": id.clone()});
    let version = schema_version();
    let string = json!({"type": "string"});
    let confidence = json!({"type": "number", "minimum": 0, "maximum": 1});
    json!({
        "scope": closed_record(&["id", "path_prefix"], json!({
            "id": id.clone(), "path_prefix": {"type": "string"}
        })),
        "sourceReference": closed_record(&["source_id"], json!({
            "source_id": id.clone(), "clause": string.clone()
        })),
        "artifactLink": closed_record(&["target_type", "target_id"], json!({
            "target_type": {"enum": ["source", "requirement", "resolution", "rule"]},
            "target_id": id.clone()
        })),
        "resolutionInput": closed_record(&["input_type", "reference", "summary"], json!({
            "input_type": {"enum": ["regulatory", "legal_advice", "commercial", "benchmark", "technical", "incident", "source_material"]},
            "reference": string.clone(), "summary": string.clone()
        })),
        "source": closed_record(
            &["schema_version", "scope_id", "id", "name", "source_type", "url"],
            json!({
                "schema_version": version.clone(), "scope_id": id.clone(), "id": id.clone(),
                "declared_by": string.clone(), "declaration_address": declaration_address(), "retired": {"type": "boolean"}, "name": string.clone(),
                "source_type": {"enum": ["policy", "document", "legislation", "company_agreement", "system_state", "external_integration", "domain_knowledge", "project_artifact", "incident", "api_spec"]},
                "url": {"type": ["string", "null"]}, "reference": string.clone(),
                "commit_pin": {
                    "type": "string", "minLength": 7, "maxLength": 64,
                    "pattern": "^[0-9A-Fa-f]+$"
                },
                "effective_date": {"type": "integer"},
                "review_date": {"type": "integer"}, "superseded_by": id.clone(),
                "supersedes": id_list.clone()
            })
        ),
        "domain": closed_record(&["schema_version", "scope_id", "id", "name"], json!({
            "schema_version": version.clone(), "scope_id": id.clone(), "id": id.clone(),
            "name": string.clone(), "description": string.clone(), "color": string.clone()
        })),
        "requirement": closed_record(
            &["schema_version", "scope_id", "id", "statement", "status"],
            json!({
                "schema_version": version.clone(), "scope_id": id.clone(), "id": id.clone(),
                "declared_by": string.clone(), "declaration_address": declaration_address(), "retired": {"type": "boolean"}, "statement": string.clone(),
                "description": string.clone(), "fog": string.clone(),
                "status": {"enum": ["active", "discovery", "refinement", "resolved"]},
                "domain_id": id.clone(),
                "source_refs": {"type": "array", "items": {"$ref": "#/$defs/sourceReference"}},
                "refines": id.clone(), "depends_on": id_list.clone(),
                "supersedes": id_list.clone(), "spawned_by": id.clone()
            })
        ),
        "boundary": closed_record(
            &["schema_version", "scope_id", "id", "requirement_id", "statement"],
            json!({
                "schema_version": version.clone(), "scope_id": id.clone(), "id": id.clone(),
                "requirement_id": id.clone(), "statement": string.clone(),
                "source_ref": {"$ref": "#/$defs/sourceReference"}
            })
        ),
        "topic": closed_record(
            &["schema_version", "scope_id", "id", "requirement_id", "title", "status", "links"],
            json!({
                "schema_version": version.clone(), "scope_id": id.clone(), "id": id.clone(),
                "requirement_id": id.clone(), "title": string.clone(),
                "status": {"enum": ["open", "explored", "closed"]},
                "links": {"type": "array", "items": {"$ref": "#/$defs/artifactLink"}}
            })
        ),
        "question": closed_record(
            &["schema_version", "scope_id", "id", "topic_id", "requirement_id", "question", "resolution_method", "status", "links"],
            json!({
                "schema_version": version.clone(), "scope_id": id.clone(), "id": id.clone(),
                "topic_id": id.clone(), "requirement_id": id.clone(), "question": string.clone(),
                "resolution_method": {"enum": ["grill", "prototype", "research", "verify", "task"]},
                "status": {"enum": ["open", "blocked_on_human", "answered"]},
                "answer": string.clone(), "resolution_id": id.clone(), "contradicts": id.clone(),
                "links": {"type": "array", "items": {"$ref": "#/$defs/artifactLink"}}
            })
        ),
        "resolution": closed_record(
            &["schema_version", "scope_id", "id", "title", "position", "rationale", "status", "inputs", "review_on"],
            json!({
                "schema_version": version.clone(), "scope_id": id.clone(), "id": id.clone(),
                "title": string.clone(), "position": string.clone(), "rationale": string.clone(),
                "status": {"enum": ["draft", "review", "proposed", "approved", "rejected", "revised", "superseded", "abandoned"]},
                "context": string.clone(), "enforcement": string.clone(), "confidence": confidence,
                "inputs": {"type": "array", "items": {"$ref": "#/$defs/resolutionInput"}},
                "made_by": string.clone(), "approved_by": string.clone(), "approved_at": {"type": "integer"},
                "requirement_ids": id_list.clone(), "supersedes": id_list.clone(),
                "superseded_by": id.clone(), "review_on": {"type": ["string", "null"]}
            })
        ),
        "rule": closed_record(
            &["schema_version", "scope_id", "id", "statement", "status", "severity"],
            json!({
                "schema_version": version.clone(), "scope_id": id.clone(), "id": id.clone(),
                "declared_by": string.clone(), "declaration_address": declaration_address(), "retired": {"type": "boolean"}, "name": string.clone(),
                "description": string.clone(),
                "statement": string.clone(), "status": {"enum": ["draft", "review", "active", "deprecated", "archived"]},
                "severity": {"enum": ["low", "medium", "high", "critical"]},
                "requirement_ids": id_list.clone(), "resolution_ids": id_list.clone(),
                "source_document": string.clone(), "source_section": string.clone()
            })
        ),
        "verificationBinding": closed_record(
            &["schema_version", "scope_id", "id", "rule_id", "key", "method", "declared_by", "file"],
            json!({
                "schema_version": version.clone(), "scope_id": id.clone(), "id": id.clone(),
                "rule_id": id.clone(), "key": string.clone(),
                "method": {"enum": ["exhaustion", "property", "examples", "conformance", "construction", "proof"]},
                "declared_by": string.clone(), "retired": {"type": "boolean"},
                "file": string.clone(), "symbol": string.clone()
            })
        ),
        "implementationBinding": closed_record(
            &["schema_version", "scope_id", "id", "rule_id", "declared_by", "file", "symbol"],
            json!({
                "schema_version": version.clone(), "scope_id": id.clone(), "id": id.clone(),
                "rule_id": id.clone(), "declared_by": string.clone(),
                "retired": {"type": "boolean"},
                "file": {"type": "string", "minLength": 1},
                "symbol": {"type": "string", "minLength": 1}
            })
        ),
        "edge": closed_record(
            &["schema_version", "scope_id", "id", "edge_type", "from_type", "from_id", "to_type", "to_id"],
            json!({
                "schema_version": version, "scope_id": id.clone(), "id": id.clone(),
                "edge_type": {"enum": ["references", "refines_into", "depends_on", "contradicts", "supersedes", "needs", "resolves", "spawns", "produces"]},
                "from_type": {"enum": ["source", "requirement", "resolution", "rule", "topic", "question"]},
                "from_id": id.clone(),
                "to_type": {"enum": ["source", "requirement", "resolution", "rule", "topic", "question"]},
                "to_id": id, "label": string
            })
        )
    })
}
