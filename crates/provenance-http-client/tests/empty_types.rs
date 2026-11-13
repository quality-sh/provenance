//! The generated types with no fields or no values read JSON as the serde
//! derive did: a request with no fields reads only an empty object, and the
//! failure of a statement check reads no value. The crate has no dependency on
//! `provenance-macros`, so these tests carry no Rule binding;
//! `info_request_input.rs` in `provenance-core` verifies the same Rule.
use provenance_http_client::types::{CheckStatementFailureStatementFailure, CloseTopicRequestData};
use serde_json::{json, Value};

fn read_request(input: Value) -> Result<(), String> {
    serde_json::from_value::<CloseTopicRequestData>(input)
        .map(drop)
        .map_err(|error| error.to_string())
}

fn read_failure(input: Value) -> String {
    match serde_json::from_value::<CheckStatementFailureStatementFailure>(input) {
        Ok(_) => "read".to_owned(),
        Err(error) => error.to_string(),
    }
}

#[test]
fn a_request_with_no_fields_reads_only_an_empty_object() {
    let values = [json!({}), json!([]), json!({"x": 1}), json!(null), json!(1)];
    let actual: Vec<_> = values
        .into_iter()
        .map(|value| (value.to_string(), read_request(value)))
        .collect();
    let expected = [
        ("{}", Ok(())),
        ("[]", Ok(())),
        ("{\"x\":1}", Err("unknown field `x`, there are no fields")),
        (
            "null",
            Err("invalid type: null, expected struct CloseTopicRequestData"),
        ),
        (
            "1",
            Err("invalid type: integer `1`, expected struct CloseTopicRequestData"),
        ),
    ]
    .map(|(input, reply)| (input.to_owned(), reply.map_err(str::to_owned)));
    assert_eq!(actual, expected);
    assert_eq!(
        serde_json::to_value(CloseTopicRequestData {}).unwrap(),
        json!({})
    );
}

#[test]
fn the_failure_of_a_statement_check_reads_no_value() {
    let values = [
        json!("unclassified"),
        json!({"unclassified": null}),
        json!({}),
        json!(1),
        json!(null),
    ];
    let actual: Vec<_> = values
        .into_iter()
        .map(|value| (value.to_string(), read_failure(value)))
        .collect();
    let expected = [
        (
            "\"unclassified\"",
            "unknown variant `unclassified`, there are no variants",
        ),
        (
            "{\"unclassified\":null}",
            "unknown variant `unclassified`, there are no variants",
        ),
        ("{}", "invalid value: map, expected map with a single key"),
        ("1", "invalid type: integer `1`, expected string or map"),
        ("null", "invalid type: null, expected string or map"),
    ]
    .map(|(input, error)| (input.to_owned(), error.to_owned()));
    assert_eq!(actual, expected);
}
