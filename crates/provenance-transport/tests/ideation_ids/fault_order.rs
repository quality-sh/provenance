//! Which fault an answer names when a line of the assertion file does not
//! parse and a proposal id is on two lines of its file.
//!
//! No Rule: the test pins an order that this change keeps. The writes read
//! the assertion file before the aggregate check of the proposal ids, and
//! the catch-up parses the assertion file before it checks the ids.

use crate::repository::{
    add_broken_line, answers, at_stored, copy_row, ideation_repository, refused, IDEATION,
};
use serde_json::json;

const BROKEN: &str = "EOF while parsing an object";

#[tokio::test]
async fn a_bad_line_in_the_assertion_file_refuses_before_a_proposal_id_on_two_lines() {
    let (repo, healthy) = ideation_repository().await;
    copy_row(&repo, "proposal_cards.jsonl", "proposal_b");
    let line = add_broken_line(&repo, "assertions.jsonl");
    let assertions = json!({"kind":"invalid_state_file",
        "file":format!("{IDEATION}/assertions.jsonl"), "line":line, "message":BROKEN});
    let fault = format!("{IDEATION}/assertions.jsonl line {line}: {BROKEN}");
    let mut expected = vec![
        refused("POST /proposals", &assertions),
        refused("POST /proposals/proposal_b/assertions", &assertions),
        refused("POST /proposals/proposal_b/dispositions", &assertions),
    ];
    expected.extend(at_stored(&healthy, &fault));
    assert_eq!(answers(&repo).await, expected);
}
