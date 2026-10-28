use assert_cmd::Command;
use serde_json::{json, Value};

fn published_schema() -> Value {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args(["schema", "show", "disposition", "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let shown: Value = serde_json::from_slice(&output.stdout).unwrap();
    let mut schema = shown["schema"].clone();
    schema["$defs"] = shown["$defs"].clone();
    schema
}

fn disposition(schema: &Value, decision: &str, rationale: Option<&str>) -> Value {
    let mut sample = json!({
        "schema_version": schema["properties"]["schema_version"]["const"],
        "scope_id": "default",
        "id": "disposition_sample",
        "proposal_id": "proposal_candidate",
        "decision": decision,
        "actor": {"identity_type": "human", "id": "reviewer"}
    });
    if let Some(text) = rationale {
        sample["rationale"] = json!(text);
    }
    sample
}

#[test]
fn published_disposition_schema_matches_rationale_rules() {
    let schema = published_schema();
    let validator = jsonschema::JSONSchema::compile(&schema).unwrap();
    for decision in ["accepted", "deferred"] {
        assert!(validator.is_valid(&disposition(&schema, decision, None)));
    }
    assert!(validator.is_valid(&disposition(&schema, "rejected", Some("Needs work."))));
    for rationale in [None, Some("  ")] {
        assert!(!validator.is_valid(&disposition(&schema, "rejected", rationale)));
    }
}
