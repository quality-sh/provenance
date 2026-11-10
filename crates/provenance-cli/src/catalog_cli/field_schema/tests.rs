//! Implementation-aid tests: no active Rule states the schema walk. The one
//! walk replaced four walkers that gave different results for some schema
//! shapes. No request schema of the catalog has these shapes today, so each
//! test pins the result of the one walk for one shape, with an in-memory
//! schema.

use super::{accepts_string, array_item_schema, closed_values, invalid_value, names_object};
use serde_json::{json, Value};

fn root() -> Value {
    json!({"$defs": {
        "Status": {"type": "string", "enum": ["active", "resolved"]},
        "Nothing": {"type": "null"},
        "Body": {"type": "object", "properties": {"text": {"type": "string"}}},
    }})
}

/// Protects the flat walk into nested unions: the values of each level are
/// listed, an inner array counts as a branch, and two arrays at two levels
/// give no item schema. The former `array_item_schema` did not go into a
/// nested union, and the former `collect` went into both union keywords.
#[test]
fn nested_unions_are_one_flat_set_of_branches() {
    let root = root();
    let values = json!({"anyOf": [
        {"anyOf": [{"enum": ["a"]}, {"oneOf": [{"const": "b"}]}]},
        {"$ref": "#/$defs/Status"},
    ]});
    assert_eq!(
        closed_values(&root, &values),
        Some(vec![
            "a".into(),
            "b".into(),
            "active".into(),
            "resolved".into()
        ])
    );
    let one_array = json!({"anyOf": [{"anyOf": [{"type": "array", "items": {"type": "string"}}]}]});
    assert_eq!(
        array_item_schema(&root, &one_array),
        Some(&json!({"type": "string"}))
    );
    let two_arrays = json!({"anyOf": [
        {"type": "array", "items": {"type": "string"}},
        {"anyOf": [{"type": "array", "items": {"type": "integer"}}]},
    ]});
    assert_eq!(array_item_schema(&root, &two_arrays), None);
    let string = json!({"anyOf": [{"anyOf": [{"type": "integer"}, {"type": ["null", "string"]}]}]});
    assert!(accepts_string(&root, &string));
}

/// Protects the removal of a `null` branch after its `$ref` resolves. The
/// former `schema_kind` looked at the type before it resolved the reference,
/// so it did not find an object here.
#[test]
fn a_reference_to_a_null_definition_is_a_null_branch() {
    let root = root();
    let object = json!({"anyOf": [{"$ref": "#/$defs/Body"}, {"$ref": "#/$defs/Nothing"}]});
    assert!(names_object(&root, &object));
    let only_null = json!({"anyOf": [{"$ref": "#/$defs/Nothing"}, {"type": "null"}]});
    assert!(!names_object(&root, &only_null));
}

/// Protects that the union decides when a schema has `type: "array"` and
/// `anyOf` together. The former `array_item_schema` read the `type` first
/// and gave the outer item schema.
#[test]
fn an_array_type_next_to_any_of_follows_the_union() {
    let root = root();
    let schema = json!({
        "type": "array",
        "items": {"type": "integer"},
        "anyOf": [{"type": "array", "items": {"type": "string"}}, {"type": "null"}],
    });
    assert_eq!(
        array_item_schema(&root, &schema),
        Some(&json!({"type": "string"}))
    );
    let no_array =
        json!({"type": "array", "items": {"type": "integer"}, "anyOf": [{"type": "null"}]});
    assert_eq!(array_item_schema(&root, &no_array), None);
}

/// Protects that `oneOf` counts only when the schema has no `anyOf`. The
/// former `collect` listed the values of both keywords.
#[test]
fn any_of_wins_over_one_of() {
    let root = root();
    let schema = json!({"anyOf": [{"enum": ["a"]}], "oneOf": [{"enum": ["b"]}]});
    assert_eq!(closed_values(&root, &schema), Some(vec!["a".into()]));
    let one_of = json!({"oneOf": [{"enum": ["b"]}, {"const": "c"}]});
    assert_eq!(
        closed_values(&root, &one_of),
        Some(vec!["b".into(), "c".into()])
    );
}

/// Protects the list for an optional closed set that keeps its `null`
/// branch: the `null` branch adds no value and does not remove the list. A
/// branch that is not `null` and not a closed set still removes the list.
#[test]
fn a_null_branch_keeps_the_accepted_values() {
    let root = root();
    let optional = json!({"anyOf": [{"$ref": "#/$defs/Status"}, {"type": "null"}]});
    assert_eq!(
        invalid_value(&root, &optional, "status").to_string(),
        "invalid value for --status; accepted values: active, resolved"
    );
    let open = json!({"anyOf": [{"$ref": "#/$defs/Status"}, {"type": "string"}]});
    assert_eq!(
        invalid_value(&root, &open, "status").to_string(),
        "invalid value for --status"
    );
}
