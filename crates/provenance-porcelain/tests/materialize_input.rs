//! Implementation aid: these tests pin how the materialize input reads each
//! JSON input and its schema, so that a change of its `Deserialize` code keeps
//! the results and the error texts. No Rule states this behaviour.
use provenance_porcelain::materialize::MaterializeInput;
use serde_json::{json, Value};

fn read_value(input: Value) -> Result<(), String> {
    serde_json::from_value::<MaterializeInput>(input)
        .map(drop)
        .map_err(|error| error.to_string())
}

#[test]
fn the_materialize_input_reads_an_empty_object_and_refuses_each_other_value() {
    let values = [
        json!({}),
        json!({"force": true}),
        json!(null),
        json!([]),
        json!([1]),
        json!("text"),
    ];
    let actual: Vec<_> = values
        .into_iter()
        .map(|value| (value.to_string(), read_value(value)))
        .collect();
    let expected = [
        ("{}".to_owned(), Ok(())),
        (
            r#"{"force":true}"#.to_owned(),
            Err("unknown field `force`, there are no fields".to_owned()),
        ),
        (
            "null".to_owned(),
            Err("invalid type: null, expected struct MaterializeInput".to_owned()),
        ),
        ("[]".to_owned(), Ok(())),
        (
            "[1]".to_owned(),
            Err("invalid length 1, expected fewer elements in array".to_owned()),
        ),
        (
            r#""text""#.to_owned(),
            Err(r#"invalid type: string "text", expected struct MaterializeInput"#.to_owned()),
        ),
    ];
    assert_eq!(actual, expected);
}

#[test]
fn the_materialize_input_schema_is_an_object_with_no_fields() {
    let schema = serde_json::to_value(schemars::schema_for!(MaterializeInput)).unwrap();
    assert_eq!(
        schema,
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "MaterializeInput",
            "description": "The `materialize` input. It has no fields: the command rebuilds the cache of the\nrepository that the consumer binds.",
            "type": "object",
            "additionalProperties": false
        })
    );
}
