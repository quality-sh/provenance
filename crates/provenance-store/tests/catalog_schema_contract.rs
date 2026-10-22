#![cfg(feature = "schema")]

use provenance_store::operations::catalog::{self, Operation};
use schemars::generate::{Contract, SchemaSettings};
use serde_json::Value;

fn derived<T: schemars::JsonSchema>(contract: Contract) -> Value {
    serde_json::to_value(
        SchemaSettings::draft2020_12()
            .with(|settings| settings.contract = contract)
            .into_generator()
            .into_root_schema_for::<T>(),
    )
    .unwrap()
}

// Implementation aid: public schema results must equal the typed schema contract.
fn check<O: Operation>() {
    let request = derived::<O::Request>(Contract::Deserialize);
    let success = derived::<O::Success>(Contract::Serialize);
    for _ in 0..2 {
        assert_eq!(catalog::operation_request_schema::<O>(), request);
        assert_eq!(catalog::operation_success_schema::<O>(), success);
    }
}

#[test]
fn repeated_schema_reads_keep_the_typed_request_and_response_contracts() {
    check::<catalog::Get>();
    check::<catalog::Search>();
    check::<catalog::GetReviewedResource>();
    check::<catalog::CreateRequirementNative>();
}
