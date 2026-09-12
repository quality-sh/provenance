#![cfg(feature = "schema")]
use provenance_core::{VerificationBinding, VerificationRun};
use provenance_store::{
    operations::TypedSpecPlan,
    state_store::{
        BeginVerificationInput, CompleteVerificationInput, CreateRuleInput, TypedSpecResult,
    },
};
use schemars::{
    generate::{Contract, SchemaSettings},
    JsonSchema,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

fn validate<T: JsonSchema>(contract: Contract, value: &Value) {
    let schema = serde_json::to_value(
        SchemaSettings::draft2020_12()
            .with(|s| s.contract = contract)
            .into_generator()
            .into_root_schema_for::<T>(),
    )
    .unwrap();
    let compiled = jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(&schema)
        .unwrap();
    if let Err(errors) = compiled.validate(value) {
        panic!(
            "{}",
            errors.map(|e| e.to_string()).collect::<Vec<_>>().join("\n")
        );
    };
}
fn round_trip<T: JsonSchema + DeserializeOwned + Serialize>(value: &Value) {
    let decoded: T = serde_json::from_value(value.clone()).unwrap();
    validate::<T>(Contract::Deserialize, value);
    validate::<T>(Contract::Serialize, &serde_json::to_value(decoded).unwrap());
}
#[test]
fn apply_and_flat_plan_use_the_real_result_types() {
    let result = json!({"declared_by":"test","created":0,"updated":0,"moved":0,"deleted":0,
        "conflicts":0,"unchanged":0,"resources":[]});
    round_trip::<TypedSpecResult>(&result);
    let mut plan = result;
    plan["affected_rules"] = json!([]);
    round_trip::<TypedSpecPlan>(&plan);
}
#[test]
fn run_requests_and_results_use_the_real_wire_types() {
    let begin = json!({"rule":"rule_a","key":"example","method":"examples","declared_by":"test"});
    serde_json::from_value::<BeginVerificationInput>(begin.clone()).unwrap();
    validate::<BeginVerificationInput>(Contract::Deserialize, &begin);
    let complete = json!({"run":"run_a","status":"passed"});
    serde_json::from_value::<CompleteVerificationInput>(complete.clone()).unwrap();
    validate::<CompleteVerificationInput>(Contract::Deserialize, &complete);
    round_trip::<VerificationRun>(
        &json!({"schema_version":1,"scope_id":"default","id":"run_a",
        "rule_id":"rule_a","binding_id":"binding_a","method":"examples",
        "declared_by":"test","status":"running","started_at":1}),
    );
    round_trip::<VerificationBinding>(
        &json!({"schema_version":1,"scope_id":"default","id":"binding_a",
        "rule_id":"rule_a","key":"example","method":"examples","declared_by":"test",
        "file":"src/lib.rs"}),
    );
}

#[test]
fn update_requests_keep_empty_clear_lists_omitted() {
    use provenance_store::state_store::{
        EditQuestionInput, UpdateBoundaryInput, UpdateDomainInput, UpdateRequirementInput,
        UpdateResolutionInput, UpdateRuleInput, UpdateSourceInput, UpdateTopicInput,
    };

    fn check<T: JsonSchema + DeserializeOwned + Serialize>() {
        let value = json!({"scope_id":"default", "id":"record_a"});
        validate::<T>(Contract::Deserialize, &value);
        let schema = SchemaSettings::draft2020_12()
            .with(|s| s.contract = Contract::Deserialize)
            .into_generator()
            .into_root_schema_for::<T>();
        let schema = serde_json::to_value(schema).unwrap();
        assert!(schema["properties"]["clear_fields"]
            .get("default")
            .is_none());
        for clear in [None, Some(json!([]))] {
            let mut input = value.clone();
            if let Some(clear) = clear {
                input["clear_fields"] = clear;
            }
            let decoded: T = serde_json::from_value(input).unwrap();
            assert!(serde_json::to_value(decoded)
                .unwrap()
                .get("clear_fields")
                .is_none());
        }
    }

    check::<UpdateSourceInput>();
    check::<UpdateResolutionInput>();
    check::<UpdateRequirementInput>();
    check::<UpdateRuleInput>();
    check::<UpdateDomainInput>();
    check::<UpdateBoundaryInput>();
    check::<UpdateTopicInput>();
    check::<EditQuestionInput>();

    let value = json!({"scope_id":"default", "id":"req_a",
        "clear_fields":["description", "fog", "domain_id"]});
    let decoded: UpdateRequirementInput = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(decoded).unwrap()["clear_fields"],
        value["clear_fields"]
    );
}

#[test]
fn rule_contract_requires_the_archive_permalink_and_validates_stamps() {
    let schema = serde_json::to_value(schemars::schema_for!(provenance_core::Rule)).unwrap();
    let compiled = jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(&schema)
        .unwrap();
    let mut rule = json!({"schema_version":2,"scope_id":"default","id":"rule_one","statement":"The system saves records.","status":"draft","severity":"medium","requirement_ids":["req_one"]});
    assert!(compiled.is_valid(&rule));
    rule["status"] = json!("archived");
    assert!(!compiled.is_valid(&rule));
    rule["archived_in_commit"] = json!({"commit":"a".repeat(40)});
    assert!(compiled.is_valid(&rule));
    rule["created"] = json!({"commit":"b".repeat(64),"at":"2026-09-12T00:00:00Z"});
    assert!(compiled.is_valid(&rule));
    rule["created"]["commit"] = json!("abc");
    assert!(!compiled.is_valid(&rule));
    rule.as_object_mut().unwrap().remove("created");
    rule["status"] = json!("active");
    assert!(!compiled.is_valid(&rule));
    assert!(schema["properties"].get("retired").is_none());
    assert!(
        serde_json::to_value(schemars::schema_for!(provenance_core::Source)).unwrap()["properties"]
            .get("archived_in_commit")
            .is_none()
    );

    let input_schema = serde_json::to_value(schemars::schema_for!(CreateRuleInput)).unwrap();
    let input_contract = jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(&input_schema)
        .unwrap();
    let mut input = json!({"scope_id":"default","id":"rule_one","statement":"The system saves records.","status":"draft","severity":"medium","requirement_ids":["req_one"],"resolution_ids":[]});
    assert!(input_contract.is_valid(&input));
    input["archived_in_commit"] = json!({"commit":"a".repeat(40)});
    assert!(!input_contract.is_valid(&input));
    input["status"] = json!("archived");
    assert!(input_contract.is_valid(&input));
}
