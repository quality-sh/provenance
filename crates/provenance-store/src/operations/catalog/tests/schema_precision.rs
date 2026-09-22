//! Bound path parameters publish exactly the schema of their typed request
//! field, so the exported constraints and the runtime deserialization limits
//! cannot drift apart.

use super::super::schema::type_schema;
use schemars::generate::Contract;
use serde_json::json;

fn catalog_definition(name: &str) -> super::super::Definition {
    super::super::definitions()
        .iter()
        .find(|definition| definition.name == name)
        .unwrap_or_else(|| panic!("missing definition {name}"))
        .clone()
}

fn path_parameter(definition: &super::super::Definition, name: &str) -> serde_json::Value {
    definition
        .parameters()
        .into_iter()
        .find(|parameter| parameter.location == "path" && parameter.name == name)
        .unwrap_or_else(|| panic!("missing path parameter {name}"))
        .schema
}

/// The exported StableId schema, taken from the type itself. The catalog
/// keeps no title in parameter positions, so the replica drops it.
fn stable_id_schema() -> serde_json::Value {
    let mut schema = type_schema::<provenance_core::StableId>(Contract::Deserialize);
    let object = schema.as_object_mut().unwrap();
    object.remove("$schema");
    object.remove("title");
    schema
}

#[test]
fn ordinary_members_publish_their_typed_id_field() {
    for name in ["get-source", "update-source"] {
        let definition = catalog_definition(name);
        assert_eq!(
            path_parameter(&definition, "id"),
            stable_id_schema(),
            "{name}"
        );
    }
}

#[test]
fn nested_members_publish_their_typed_id_fields() {
    let definition = catalog_definition("get-requirement-history-entry");
    for name in ["id", "entry_id"] {
        assert_eq!(
            path_parameter(&definition, name),
            stable_id_schema(),
            "{name}"
        );
    }
    let fact = catalog_definition("get-proposal-assertion");
    for name in ["id", "fact_id"] {
        assert_eq!(path_parameter(&fact, name), stable_id_schema(), "{name}");
    }
}

#[test]
fn addressed_discussion_ids_publish_their_typed_leaves() {
    let message = catalog_definition("sources-get-discussion-message");
    for name in ["id", "discussion_id", "message_id"] {
        assert_eq!(path_parameter(&message, name), stable_id_schema(), "{name}");
    }
    let write = catalog_definition("sources-create-discussion-message");
    assert_eq!(path_parameter(&write, "discussion_id"), stable_id_schema());
}

#[test]
fn the_evidence_side_publishes_its_closed_enum() {
    let evidence = catalog_definition("get-requirement-history-evidence");
    assert_eq!(
        path_parameter(&evidence, "side"),
        json!({"description": "The published evidence side of a review outcome. The enum is the whole\ncontract: wire values outside `before` and `after` are unrepresentable, so\nthe request carries no second string validation.", "enum": ["before", "after"], "type": "string"}),
    );
    let parameter = evidence
        .parameters()
        .into_iter()
        .find(|parameter| parameter.name == "side")
        .unwrap();
    assert!(super::super::parse_parameter_value(&parameter, "sideways").is_err());
    assert_eq!(
        super::super::parse_parameter_value(&parameter, "before").unwrap(),
        json!("before")
    );
}

#[test]
fn history_evidence_requests_refuse_invented_sides_at_the_type() {
    for side in ["sideways", "", "BEFORE"] {
        let request: Result<crate::operations::catalog::HistoryEvidenceRequest, _> =
            serde_json::from_value(
                json!({"requirement_id": "requirement_a", "entry_id": "entry_a", "side": side}),
            );
        assert!(request.is_err(), "side {side} is not a published value");
    }
    let request: crate::operations::catalog::HistoryEvidenceRequest =
        serde_json::from_value(json!({
            "requirement_id": "requirement_a", "entry_id": "entry_a", "side": "after"
        }))
        .unwrap();
    assert_eq!(
        request.side,
        crate::operations::catalog::ReviewEvidenceSide::After
    );
}
