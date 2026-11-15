//! A `document_catch_up_failed` failure reads its `file` and `line` together
//! or not at all: a body with one of the two keys is refused, and a body with
//! both keys or none reads and writes again unchanged.

use provenance_core::protocol::read_failure::ReadFailure;
use provenance_macros::verifies;
use serde_json::{json, Value};

/// The read error of a body with one of the two keys.
const ONE_KEY: &str = "a failure gives file and line together, or neither";

/// The failure that `body` reads as, written again, or the read error.
fn read(body: &Value) -> Result<Value, String> {
    let failure = serde_json::from_value::<ReadFailure>(body.clone());
    let failure = failure.map_err(|error| error.to_string())?;
    Ok(serde_json::to_value(failure).unwrap())
}

#[test]
#[verifies("rule_document_catch_up_refusal_gives_typed_cause", examples)]
fn a_failure_with_the_file_and_the_line_or_neither_reads_unchanged() {
    let located = json!({
        "kind": "document_catch_up_failed",
        "file": "scopes/default/requirements/req.jsonl",
        "line": 1,
        "message": "expected value"
    });
    let unlocated = json!({
        "kind": "document_catch_up_failed",
        "message": "depends_on forms a cycle: req_c -> req_d -> req_c"
    });

    for body in [located, unlocated] {
        assert_eq!(read(&body), Ok(body.clone()));
    }
}

#[test]
#[verifies("rule_document_catch_up_refusal_gives_typed_cause", examples)]
fn a_failure_with_only_the_file_or_only_the_line_is_refused() {
    let only_file = json!({
        "kind": "document_catch_up_failed",
        "file": "scopes/default/requirements/req.jsonl",
        "message": "expected value"
    });
    let only_line = json!({
        "kind": "document_catch_up_failed",
        "line": 1,
        "message": "expected value"
    });

    for body in [only_file, only_line] {
        assert_eq!(read(&body), Err(ONE_KEY.to_owned()), "{body}");
    }
}
