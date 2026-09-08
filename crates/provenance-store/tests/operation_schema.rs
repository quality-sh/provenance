#![cfg(feature = "schema")]
use provenance_core::{VerificationBinding, VerificationRun};
use provenance_store::{
    operations::TypedSpecPlan,
    state_store::{BeginVerificationInput, CompleteVerificationInput, TypedSpecResult},
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
    let result = json!({"declared_by":"test","created":0,"updated":0,"moved":0,"retired":0,
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
