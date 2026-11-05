//! Implementation aids for the flat tool input.

use super::*;

fn input() -> ToolInput {
    ToolInput::new(Some("list"))
        .with_action(
            "list",
            json!({"properties": {"parent": {"type": ["string", "null"]}}}),
        )
        .with_action(
            "start",
            json!({
                "properties": {"parent": {"type": "string"}, "body": {"$ref": "#/$defs/Body"}},
                "required": ["parent", "body"],
                "$defs": {"Body": {"type": "string"}}
            }),
        )
}

fn arguments(value: &Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap()
}

#[test]
fn the_schema_is_one_flat_object() {
    assert_eq!(
        input().schema(),
        json!({
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ["list", "start"], "default": "list"},
                "parent": {"type": ["string", "null"], "description": "Actions: list, start."},
                "body": {"$ref": "#/$defs/Body", "description": "Actions: start."}
            },
            "required": [],
            "additionalProperties": false,
            "$defs": {"Body": {"type": "string"}}
        })
    );
}

#[test]
fn a_call_without_an_action_selects_the_default() {
    let mut call = arguments(&json!({"parent": "req_a"}));
    assert_eq!(input().select(&mut call), Ok("list"));
}

#[test]
fn a_field_of_another_action_is_refused_by_name() {
    let refusal = input()
        .select(&mut arguments(&json!({"action": "list", "body": "Hello."})))
        .unwrap_err();
    assert_eq!(
        refusal.detail(),
        json!({
            "kind": "invalid_input",
            "field": "body",
            "reason": "unknown_field",
            "message": "the list action does not take body; it takes: parent"
        })
    );
}

#[test]
fn a_missing_field_is_refused_by_name() {
    let refusal = input()
        .select(&mut arguments(
            &json!({"action": "start", "parent": "req_a"}),
        ))
        .unwrap_err();
    assert_eq!(
        (refusal.field.as_str(), refusal.reason),
        ("body", InvalidInputReason::Required)
    );
}

#[test]
fn an_unknown_action_is_refused() {
    let refusal = ToolInput::new(None)
        .with_action("list", json!({}))
        .select(&mut arguments(&json!({"action": "stop"})))
        .unwrap_err();
    assert_eq!(
        refusal.message,
        "unknown action stop; the actions are: list"
    );
    let refusal = ToolInput::new(None)
        .with_action("list", json!({}))
        .select(&mut Map::new())
        .unwrap_err();
    assert_eq!(
        (refusal.field.as_str(), refusal.reason),
        ("action", InvalidInputReason::Required)
    );
}

#[test]
fn a_field_of_two_actions_has_one_schema() {
    assert_eq!(
        merged(&json!({"type": "string"}), &json!({"type": "object"})),
        json!({"anyOf": [{"type": "string"}, {"type": "object"}]})
    );
    assert_eq!(
        merged(
            &json!({"$ref": "#/$defs/A"}),
            &json!({"anyOf": [{"$ref": "#/$defs/A"}, {"type": "null"}], "default": null})
        ),
        json!({"anyOf": [{"$ref": "#/$defs/A"}, {"type": "null"}], "default": null})
    );
    assert_eq!(
        merged(
            &json!({"type": "string", "description": "The record ID."}),
            &json!({"type": "string"})
        ),
        json!({"type": "string", "description": "The record ID."})
    );
}
