#![cfg(feature = "schema")]
use provenance_ste100::StandardIssue;
use schemars::generate::{Contract, SchemaSettings};
use serde_json::json;

#[test]
fn issue_schema_accepts_only_the_serialized_integer_nine() {
    for contract in [Contract::Serialize, Contract::Deserialize] {
        let schema = SchemaSettings::draft2020_12()
            .with(|s| s.contract = contract.clone())
            .into_generator()
            .into_root_schema_for::<StandardIssue>();
        let value = serde_json::to_value(schema).unwrap();
        let validator = jsonschema::JSONSchema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .compile(&value)
            .unwrap();
        assert!(validator.is_valid(&serde_json::to_value(StandardIssue::Nine).unwrap()));
        for invalid in [json!(8), json!(10), json!("9"), json!("Nine"), json!(null)] {
            assert!(
                !validator.is_valid(&invalid),
                "accepted {invalid} in {contract:?}"
            );
        }
    }
}

#[test]
fn analyzer_output_validates_against_its_serialize_schema() {
    use provenance_ste100::{check_descriptive, Report};
    let value =
        serde_json::to_value(check_descriptive("The system works; it stores data.")).unwrap();
    let schema = SchemaSettings::draft2020_12()
        .with(|s| s.contract = Contract::Serialize)
        .into_generator()
        .into_root_schema_for::<Report>();
    let schema = serde_json::to_value(schema).unwrap();
    let compiled = jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(&schema)
        .unwrap();
    assert!(compiled.is_valid(&value));
    assert_eq!(value["issue"], 9);
    assert!(!value["findings"].as_array().unwrap().is_empty());
}
