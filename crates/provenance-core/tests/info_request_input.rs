//! Implementation aid: these tests pin how the info request reads each JSON
//! input and its schema, so that a change of its `Deserialize` code keeps the
//! results and the error texts. No Rule states this behaviour.
use provenance_core::protocol::repository::InfoRequest;
use serde_json::{json, Value};

fn read_value(input: Value) -> Result<(), String> {
    serde_json::from_value::<InfoRequest>(input)
        .map(drop)
        .map_err(|error| error.to_string())
}

fn read_text(input: &str) -> Result<(), String> {
    serde_json::from_str::<InfoRequest>(input)
        .map(drop)
        .map_err(|error| error.to_string())
}

#[test]
fn the_info_request_reads_an_empty_object_and_refuses_each_other_value() {
    let values = [
        json!({}),
        json!({"repository": "a"}),
        json!({"b": 1, "a": 2}),
        json!(null),
        json!([]),
        json!([1]),
        json!("text"),
        json!(1),
        json!(true),
    ];
    let actual: Vec<_> = values
        .into_iter()
        .map(|value| (value.to_string(), read_value(value)))
        .collect();
    let expected = [
        ("{}".to_owned(), Ok(())),
        (
            r#"{"repository":"a"}"#.to_owned(),
            Err("unknown field `repository`, there are no fields".to_owned()),
        ),
        (
            r#"{"a":2,"b":1}"#.to_owned(),
            Err("unknown field `a`, there are no fields".to_owned()),
        ),
        (
            "null".to_owned(),
            Err("invalid type: null, expected struct InfoRequest".to_owned()),
        ),
        ("[]".to_owned(), Ok(())),
        (
            "[1]".to_owned(),
            Err("invalid length 1, expected fewer elements in array".to_owned()),
        ),
        (
            r#""text""#.to_owned(),
            Err(r#"invalid type: string "text", expected struct InfoRequest"#.to_owned()),
        ),
        (
            "1".to_owned(),
            Err("invalid type: integer `1`, expected struct InfoRequest".to_owned()),
        ),
        (
            "true".to_owned(),
            Err("invalid type: boolean `true`, expected struct InfoRequest".to_owned()),
        ),
    ];
    assert_eq!(actual, expected);
}

#[test]
fn the_info_request_gives_the_position_of_a_refusal_in_json_text() {
    let texts = ["{}", r#"{"a":1}"#, "[]", "null", "{"];
    let actual: Vec<_> = texts
        .into_iter()
        .map(|text| (text, read_text(text)))
        .collect();
    let expected = [
        ("{}", Ok(())),
        (
            r#"{"a":1}"#,
            Err("unknown field `a`, there are no fields at line 1 column 4".to_owned()),
        ),
        ("[]", Ok(())),
        (
            "null",
            Err("invalid type: null, expected struct InfoRequest at line 1 column 4".to_owned()),
        ),
        (
            "{",
            Err("EOF while parsing an object at line 1 column 1".to_owned()),
        ),
    ];
    assert_eq!(actual, expected);
}

#[cfg(feature = "schema")]
#[test]
fn the_info_request_schema_is_an_object_with_no_fields() {
    let schema = serde_json::to_value(
        schemars::generate::SchemaSettings::draft2020_12()
            .with(|settings| settings.contract = schemars::generate::Contract::Deserialize)
            .into_generator()
            .into_root_schema_for::<InfoRequest>(),
    )
    .unwrap();
    assert_eq!(
        schema,
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "InfoRequest",
            "type": "object",
            "additionalProperties": false
        })
    );
}
