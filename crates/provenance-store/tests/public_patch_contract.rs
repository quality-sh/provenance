#![cfg(feature = "schema")]

use provenance_store::operations::catalog::{self, Definition};
use serde_json::{json, Value};

fn definition(name: &str) -> &'static Definition {
    catalog::definitions()
        .iter()
        .find(|definition| definition.name == name)
        .unwrap()
}

fn validator(name: &str) -> jsonschema::JSONSchema {
    jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(definition(name).request_schema().unwrap())
        .unwrap()
}

#[test]
fn public_patch_schemas_apply_registered_null_semantics() {
    let source = validator("update-source");
    assert!(source.is_valid(&json!({"data": {}})));
    assert!(source.is_valid(&json!({"data": {"url": null}})));
    assert!(!source.is_valid(&json!({"data": {"name": null}})));
    assert!(!source.is_valid(&json!({"data": {"clear_fields": ["url"]}})));

    let requirement = validator("update-requirement");
    assert!(requirement.is_valid(&json!({"data": {"fog": null}})));
    assert!(!requirement.is_valid(&json!({"data": {"statement": null}})));

    let contribution = validator("update-contribution");
    assert!(contribution.is_valid(&json!({"data": {}})));
    assert!(!contribution.is_valid(&json!({
        "data": {"strongest_finding": null}
    })));
}

#[test]
fn public_patch_schemas_and_mcp_inputs_hide_native_clear_fields() {
    for definition in catalog::definitions()
        .iter()
        .filter(|definition| definition.method == catalog::HttpMethod::Patch)
    {
        let request = definition.request_schema().unwrap();
        assert!(
            request
                .pointer("/properties/data/properties/clear_fields")
                .is_none(),
            "{} publishes native clear_fields: {request}",
            definition.name
        );
        let mcp = definition.mcp_input_schema();
        assert!(
            mcp.pointer("/properties/data/properties/clear_fields")
                .is_none(),
            "{} publishes native clear_fields through MCP: {mcp}",
            definition.name
        );
    }
}

#[test]
fn nullable_projection_changes_only_the_public_contract() {
    let native = serde_json::to_value(schemars::schema_for!(
        provenance_store::state_store::UpdateSourceInput
    ))
    .unwrap();
    assert!(native["properties"].get("clear_fields").is_some());
    let decoded: provenance_store::state_store::UpdateSourceInput = serde_json::from_value(json!({
        "scope_id": "default",
        "id": "source_a",
        "clear_fields": ["url"]
    }))
    .unwrap();
    assert_eq!(
        serde_json::to_value(decoded).unwrap()["clear_fields"],
        json!(["url"])
    );
}
