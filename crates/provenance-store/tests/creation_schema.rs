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
        .compile(definition.request_schema().unwrap())
        .unwrap();
    let value =
        json!({"data":{"id":"source_test","name":"Policy","source_type":"policy","supersedes":[]}});
    assert!(validator.is_valid(&value));
    let mut null = value.clone();
    null["data"]["origin_thread"] = serde_json::Value::Null;
    assert!(validator.is_valid(&null));
    for field in ["origin_thread", "source_type", "supersedes"] {
        let mut invalid = value.clone();
        invalid["data"][field] = json!(42);
        assert!(!validator.is_valid(&invalid), "{field}");
        invalid["data"]["scope_id"] = json!("default");
        assert!(
            serde_json::from_value::<provenance_store::state_store::CreateSourceInput>(
                invalid["data"].clone()
            )
            .is_err()
        );
    }
    let mut scoped = value.clone();
    scoped["data"]["scope_id"] = json!("default");
    assert!(!validator.is_valid(&scoped));
    let mut extra = value;
    extra["data"]["discussion_id"] = json!("thread_hidden");
    assert!(!validator.is_valid(&extra));
    extra["data"]["scope_id"] = json!("default");
    assert!(
        serde_json::from_value::<provenance_store::state_store::CreateSourceInput>(
            extra["data"].clone()
        )
        .is_err()
    );
}
