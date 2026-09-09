//! The ideation contract describes the existing record and input shapes, so
//! legacy and current JSON keep round-tripping through them unchanged.
#![cfg(feature = "schema")]
use provenance_store::operations::catalog;
use schemars::{
    generate::{Contract, SchemaSettings},
    JsonSchema,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

fn compiled(schema: &Value) -> jsonschema::JSONSchema {
    jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(schema)
        .unwrap()
}

fn request_validator(operation: &str) -> jsonschema::JSONSchema {
    let definitions = catalog::definitions();
    let definition = definitions
        .iter()
        .find(|entry| entry.name == operation)
        .unwrap();
    compiled(&definition.request_schema)
}

fn round_trip<T: JsonSchema + DeserializeOwned + Serialize>(value: &Value) {
    let decoded: T = serde_json::from_value(value.clone()).unwrap();
    validate::<T>(Contract::Deserialize, value);
    validate::<T>(Contract::Serialize, &serde_json::to_value(decoded).unwrap());
}

fn validate<T: JsonSchema>(contract: Contract, value: &Value) {
    let schema = serde_json::to_value(
        SchemaSettings::draft2020_12()
            .with(|s| s.contract = contract)
            .into_generator()
            .into_root_schema_for::<T>(),
    )
    .unwrap();
    if let Err(errors) = compiled(&schema).validate(value) {
        panic!(
            "{}",
            errors.map(|e| e.to_string()).collect::<Vec<_>>().join("\n")
        );
    };
}

/// The shipped-v1 proposal row layout: camelCase aliases, optional fields
/// absent, and the fields a proposal author may leave out.
#[test]
fn legacy_and_current_proposal_json_roundtrips_without_new_fields() {
    let legacy = json!({
        "schema_version": 2, "scope_id": "default", "proposalId": "proposal_old",
        "proposalKey": "overtime", "proposalType": "requirement_candidate",
        "title": "Overtime", "summary": "Clarify overtime.",
        "traceability": {
            "target": {"artifactType": "requirement", "artifactId": "req_a"},
            "sourceIds": [], "evidenceReferences": [], "supportingClaimIds": []
        },
        "promotionState": "proposed"
    });
    let current = json!(serde_json::from_value::<provenance_core::ProposalCard>(legacy).unwrap());
    assert_eq!(current["id"], "proposal_old");
    assert_eq!(current["proposal_type"], "requirement_candidate");
    assert_eq!(
        current["traceability"]["target"]["artifact_type"],
        "requirement"
    );
    for absent in ["confidence", "builds_on", "duplicate_of", "superseded_by"] {
        assert!(current.get(absent).is_none(), "{absent}");
    }
    assert_eq!(
        json!(serde_json::from_value::<provenance_core::ProposalCard>(current.clone()).unwrap()),
        current
    );
    round_trip::<provenance_core::ProposalCard>(&current);
}

#[test]
fn disposition_json_keeps_absent_and_null_external_action_distinct() {
    let without_action = json!({
        "schema_version": 2, "scope_id": "default", "id": "disposition_one",
        "proposal_id": "proposal_a", "decision": "rejected", "rationale": "Reviewed",
        "actor": {"identity_type": "human", "id": "reviewer"}
    });
    let decoded: provenance_core::DispositionRecord =
        serde_json::from_value(without_action.clone()).unwrap();
    assert!(decoded.external_action.is_none());
    assert_eq!(json!(decoded), without_action);
    let mut explicit_null = without_action;
    explicit_null["external_action"] = Value::Null;
    // The record's custom deserializer distinguishes absent from explicit
    // null, so the stored-record shape still refuses a null field.
    assert!(
        serde_json::from_value::<provenance_core::DispositionRecord>(explicit_null).is_err(),
        "an explicit null external_action is not a stored disposition shape"
    );
    let with_action = json!({
        "schema_version": 2, "scope_id": "default", "id": "disposition_two",
        "proposal_id": "proposal_a", "decision": "accepted", "rationale": "Accepted",
        "actor": {"identity_type": "human", "id": "reviewer", "name": "Reviewer"},
        "canonical_artifact": {"artifact_type": "requirement", "artifact_id": "req_a"},
        "external_action": {"system": "board", "scope": "team", "kind": "ticket", "key": "T-1"}
    });
    round_trip::<provenance_core::DispositionRecord>(&with_action);
}

#[test]
fn assertion_json_roundtrips_with_legacy_claim_ids() {
    let assertion = json!({
        "schema_version": 2, "scope_id": "default", "id": "assertion_a",
        "proposal_id": "proposal_a", "synthesis_packet_id": "synthesis_a",
        "supporting_claim_ids": ["claim_a", "claim_b"]
    });
    round_trip::<provenance_core::AssertionRecord>(&assertion);
}

#[test]
fn creation_requests_are_closed_and_keep_optional_fields_optional() {
    let proposals = request_validator("create-proposal");
    let minimal = json!({"context":{"repository":"selected","scope":"default"},"request":{
        "scope_id":"default","id":"proposal_new","proposal_key":"overtime",
        "proposal_type":"requirement_candidate","title":"T","summary":"S",
        "traceability":{"target":{"artifact_type":"requirement","artifact_id":"req_a"},
            "source_ids":[],"evidence_references":[],"supporting_claim_ids":[]},
        "builds_on":[],"promotion_state":"proposed"}});
    assert!(proposals.is_valid(&minimal));
    let mut confidence = minimal.clone();
    confidence["request"]["confidence"] = json!(0.5);
    assert!(proposals.is_valid(&confidence));
    let mut out_of_range = confidence.clone();
    out_of_range["request"]["confidence"] = json!("high");
    assert!(!proposals.is_valid(&out_of_range));
    let dispositions = request_validator("create-disposition");
    let disposition = json!({"context":{"repository":"selected","scope":"default"},"request":{
        "scope_id":"default","id":"disposition_new","proposal_id":"proposal_a",
        "decision":"rejected","rationale":"Reviewed",
        "actor":{"identity_type":"human","id":"reviewer"}}});
    assert!(dispositions.is_valid(&disposition));
    let mut null_action = disposition.clone();
    null_action["request"]["external_action"] = Value::Null;
    assert!(dispositions.is_valid(&null_action));
    let mut unknown_decision = disposition.clone();
    unknown_decision["request"]["decision"] = json!("postponed");
    assert!(!dispositions.is_valid(&unknown_decision));
    let assertions = request_validator("create-assertion");
    let assertion = json!({"context":{"repository":"selected","scope":"default"},"request":{
        "scope_id":"default","id":"assertion_new","proposal_id":"proposal_a",
        "synthesis_packet_id":"synthesis_a","supporting_claim_ids":["claim_a"]}});
    assert!(assertions.is_valid(&assertion));
    for (validator, mut extra, field) in [
        (proposals, minimal, "discussion_id"),
        (dispositions, disposition, "rationale_note"),
        (assertions, assertion, "confidence"),
    ] {
        extra["request"][field] = json!("extra");
        assert!(!validator.is_valid(&extra), "{field}");
    }
}

#[test]
fn list_requests_take_a_scope_and_a_null_request() {
    for operation in ["list-proposals", "list-dispositions", "list-assertions"] {
        let validator = request_validator(operation);
        let valid = json!({"context":{"repository":"selected","scope":"default"},"request":null});
        assert!(validator.is_valid(&valid), "{operation}");
        let mut scoped = valid.clone();
        scoped["request"] = json!({"limit": 5});
        assert!(!validator.is_valid(&scoped), "{operation}");
        let mut unscoped = valid;
        unscoped["context"] = json!({"repository":"selected"});
        assert!(!validator.is_valid(&unscoped), "{operation}");
    }
}

#[test]
fn list_results_are_arrays_and_mcp_wraps_them_in_result() {
    let definitions = catalog::definitions();
    for name in ["list-proposals", "list-dispositions", "list-assertions"] {
        let definition = definitions.iter().find(|entry| entry.name == name).unwrap();
        assert_eq!(definition.success_schema["type"], "array", "{name}");
        assert!(!definition.mutates, "{name}");
        let output = definition.mcp_output_schema();
        assert_eq!(output["type"], "object");
        assert_eq!(output["properties"]["result"]["type"], "array");
    }
    for name in ["create-proposal", "create-assertion", "create-disposition"] {
        let definition = definitions.iter().find(|entry| entry.name == name).unwrap();
        assert!(definition.mutates, "{name}");
    }
}
