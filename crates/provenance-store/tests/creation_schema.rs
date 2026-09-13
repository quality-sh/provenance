#![cfg(feature = "schema")]
use provenance_store::operations::catalog;
use serde_json::json;

#[test]
fn creation_contract_preserves_store_scope_and_closed_input_fields() {
    let definitions = catalog::definitions();
    let definition = definitions
        .iter()
        .find(|entry| entry.name == "create-source")
        .unwrap();
    let validator = jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(&definition.request_schema)
        .unwrap();
    let value = json!({"context":{"repository":"selected","scope":"default"},"request":{"scope_id":"default","id":"source_test","name":"Policy","source_type":"policy","supersedes":[]}});
    assert!(validator.is_valid(&value));
    let mut null = value.clone();
    null["request"]["origin_thread"] = serde_json::Value::Null;
    assert!(validator.is_valid(&null));
    for field in ["origin_thread", "scope_id", "source_type", "supersedes"] {
        let mut invalid = value.clone();
        invalid["request"][field] = json!(42);
        assert!(!validator.is_valid(&invalid), "{field}");
        assert!(
            serde_json::from_value::<provenance_store::state_store::CreateSourceInput>(
                invalid["request"].clone()
            )
            .is_err()
        );
    }
    let mut extra = value;
    extra["request"]["discussion_id"] = json!("thread_hidden");
    assert!(!validator.is_valid(&extra));
    assert!(
        serde_json::from_value::<provenance_store::state_store::CreateSourceInput>(
            extra["request"].clone()
        )
        .is_err()
    );
}
