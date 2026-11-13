//! Implementation aid: these tests pin how the failure of a statement check
//! reads JSON and its schema, so that a change of the type keeps the results
//! and the error texts of an enum with no variants. No Rule states this
//! behaviour. The catalog of operations needs the `host` feature.
#![cfg(feature = "host")]
use provenance_store::operations::catalog::{CheckStatement, Operation};
use serde_json::{json, Value};

type StatementFailure = <CheckStatement as Operation>::Failure;

fn read(input: Value) -> String {
    match serde_json::from_value::<StatementFailure>(input) {
        Ok(failure) => match failure.0 {},
        Err(error) => error.to_string(),
    }
}

#[test]
fn the_statement_failure_refuses_each_json_value() {
    let values = [
        json!("unclassified"),
        json!({"unclassified": null}),
        json!({}),
        json!({"a": 1, "b": 2}),
        json!(1),
        json!(null),
        json!([]),
        json!(true),
    ];
    let actual: Vec<_> = values
        .into_iter()
        .map(|value| (value.to_string(), read(value)))
        .collect();
    let expected = [
        (
            r#""unclassified""#,
            "unknown variant `unclassified`, there are no variants",
        ),
        (
            r#"{"unclassified":null}"#,
            "unknown variant `unclassified`, there are no variants",
        ),
        ("{}", "invalid value: map, expected map with a single key"),
        (
            r#"{"a":1,"b":2}"#,
            "invalid value: map, expected map with a single key",
        ),
        ("1", "invalid type: integer `1`, expected string or map"),
        ("null", "invalid type: null, expected string or map"),
        ("[]", "invalid type: sequence, expected string or map"),
        (
            "true",
            "invalid type: boolean `true`, expected string or map",
        ),
    ]
    .map(|(input, error)| (input.to_owned(), error.to_owned()));
    assert_eq!(actual, expected);
}

#[cfg(feature = "schema")]
#[test]
fn the_statement_failure_schema_refuses_each_value() {
    let schema = serde_json::to_value(
        schemars::generate::SchemaSettings::draft2020_12()
            .into_generator()
            .into_root_schema_for::<StatementFailure>(),
    )
    .unwrap();
    assert_eq!(
        schema,
        json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "title": "StatementFailure", "not": {}})
    );
}
