//! A document read through the MCP `api` tool whose catch-up fails on a
//! state line that does not parse gives the file, the line and the parse
//! message of that line.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_macros::verifies;
use serde_json::{json, Value};
use support::fixture_host::read_only;
use support::mcp::{call, Client, Session};
use support::records::Repository;
use support::state_lines::give_refines_a_string;

/// The `is_error` flag and the structured content of one document read.
async fn read_document(client: &Client) -> (Option<bool>, Value) {
    let path = json!({"path": "requirements/req_shared/document"});
    let result = call(client, "api", path).await;
    (
        result.is_error,
        result.structured_content.unwrap_or(Value::Null),
    )
}

#[tokio::test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
#[verifies("rule_document_catch_up_refusal_gives_typed_cause", examples)]
async fn a_document_read_gives_the_line_that_stops_its_catch_up() {
    let repo = Repository::new("The shared graph is readable.");
    let session = Session::start(read_only(&repo)).await;
    let (first_error, first) = read_document(&session.client).await;
    assert_ne!(first_error, Some(true), "{first}");
    give_refines_a_string(repo.dir.path(), "req_shared");

    let (is_error, refused) = read_document(&session.client).await;

    session.close().await;
    assert_eq!(is_error, Some(true));
    assert_eq!(
        refused,
        json!({
            "error": {
                "kind": "document_catch_up_failed",
                "file": "scopes/default/requirements/req.jsonl",
                "line": 1,
                "message": "invalid type: string \"req_parent\", expected a sequence"
            },
            "meta": {}
        })
    );
}
