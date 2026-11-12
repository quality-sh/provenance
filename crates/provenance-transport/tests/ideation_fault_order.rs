//! Which fault an answer names when two ideation files each hold a fault:
//! the writes read the files in the order of each writer, and the catch-up
//! parses the proposal, assertion and disposition files before it checks
//! the ids.
//!
//! No Rule: the tests pin answers that this change keeps.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod ideation;
    pub mod ideation_ids;
    pub mod records;
    pub mod resource_http;
}

use serde_json::{json, Value};
use support::ideation_ids::{
    add_broken_line, answers, at_stored, copy_row, ideation_repository, refused, IDEATION,
};

const BROKEN: &str = "EOF while parsing an object";

/// The refusal of the line `line` of the file `name`, which does not parse.
fn broken(name: &str, line: usize) -> Value {
    json!({"kind":"invalid_state_file", "file":format!("{IDEATION}/{name}"), "line":line,
        "message":BROKEN})
}

#[tokio::test]
async fn a_bad_line_in_the_proposal_file_refuses_before_one_in_the_assertion_file() {
    let (repo, healthy) = ideation_repository().await;
    let line = add_broken_line(&repo, "proposal_cards.jsonl");
    let assertion_line = add_broken_line(&repo, "assertions.jsonl");
    let proposals = broken("proposal_cards.jsonl", line);
    let fault = format!("{IDEATION}/proposal_cards.jsonl line {line}: {BROKEN}");
    let lists = at_stored(&healthy[1..], &fault);
    // The assertion writer reads its own file first. The read of the
    // Requirement reads the proposal file.
    let mut expected = vec![
        refused("POST /proposals", &proposals),
        refused(
            "POST /proposals/proposal_b/assertions",
            &broken("assertions.jsonl", assertion_line),
        ),
        refused("POST /proposals/proposal_b/dispositions", &proposals),
        refused("GET /requirements/req_shared", &proposals),
    ];
    expected.extend(lists);
    assert_eq!(answers(&repo).await, expected);
}

/// The writes read the assertion file before an aggregate check of the
/// proposal ids. The read of the Requirement does not read the assertion
/// file and does not check the proposal ids.
#[tokio::test]
async fn a_bad_line_in_the_assertion_file_refuses_before_a_proposal_id_on_two_lines() {
    let (repo, healthy) = ideation_repository().await;
    copy_row(&repo, "proposal_cards.jsonl", "proposal_a");
    let line = add_broken_line(&repo, "assertions.jsonl");
    let assertions = broken("assertions.jsonl", line);
    let fault = format!("{IDEATION}/assertions.jsonl line {line}: {BROKEN}");
    let mut expected = vec![
        refused("POST /proposals", &assertions),
        refused("POST /proposals/proposal_b/assertions", &assertions),
        refused("POST /proposals/proposal_b/dispositions", &assertions),
    ];
    expected.extend(at_stored(&healthy, &fault));
    assert_eq!(answers(&repo).await, expected);
}
