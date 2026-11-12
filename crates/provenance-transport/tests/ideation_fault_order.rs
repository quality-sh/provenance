//! When the proposal file and the assertion file each hold a line that does
//! not parse, each answer names the proposal file: the reads of the three
//! ideation files come in the order proposals, assertions, dispositions.
//!
//! No Rule: the test pins an order that this change keeps.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod ideation_ids;
    pub mod records;
    pub mod resource_http;
}

use serde_json::{json, Value};
use support::ideation_ids::{add_broken_line, answers, ideation_repository, IDEATION};

#[tokio::test]
async fn a_bad_line_in_the_proposal_file_refuses_before_one_in_the_assertion_file() {
    let repo = ideation_repository().await;
    let line = add_broken_line(&repo, "proposal_cards.jsonl");
    let assertion_line = add_broken_line(&repo, "assertions.jsonl");
    let message = "EOF while parsing an object";
    let at = |name: &str, line: usize| {
        json!({"kind":"invalid_state_file", "file":format!("{IDEATION}/{name}"), "line":line,
            "message":message})
    };
    let refused = |request: &str, error: Value| {
        json!({"request":request, "status":500, "error":error,
            "freshness_error":null})
    };
    let stale = |request: &str| {
        json!({"request":request, "status":200, "error":null,
            "freshness_error":format!("{IDEATION}/proposal_cards.jsonl line {line}: {message}")})
    };
    let proposals = at("proposal_cards.jsonl", line);
    // The assertion writer reads its own file first.
    let expected = vec![
        refused("POST /proposals", proposals.clone()),
        refused(
            "POST /proposals/proposal_b/assertions",
            at("assertions.jsonl", assertion_line),
        ),
        refused("POST /proposals/proposal_b/dispositions", proposals.clone()),
        refused("GET /requirements/req_shared", proposals),
        stale("GET /proposals"),
        stale("GET /assertions"),
        stale("GET /dispositions"),
    ];
    assert_eq!(answers(&repo).await, expected);
}
