//! A proposal, assertion or disposition id is immutable. When a hand edit
//! or a branch merge puts one id on two lines of its file, each ideation
//! write refuses with `invalid_state_file` at the second line. A resource
//! read loads the proposals and the dispositions of the review cycle, and
//! refuses in the same way. A list read answers from the projection and
//! gives the same line as the fault of its catch-up.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod ideation_ids;
    pub mod records;
    pub mod resource_http;
}

use provenance_macros::verifies;
use serde_json::json;
use support::ideation_ids::{answers, copy_row, ideation_repository, IDEATION};

#[tokio::test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
async fn an_ideation_id_on_two_lines_is_located_at_the_second_line() {
    for (name, kind, id) in [
        ("proposal_cards.jsonl", "proposal", "proposal_a"),
        ("assertions.jsonl", "assertion", "assertion_a"),
        ("dispositions.jsonl", "disposition", "disposition_a"),
    ] {
        let repo = ideation_repository().await;
        let (first, second) = copy_row(&repo, name, id);
        let file = format!("{IDEATION}/{name}");
        let message =
            format!("duplicate immutable {kind} id {id} at line {first} and at line {second}");
        let refusal = json!({"kind":"invalid_state_file", "file":file, "line":second,
            "message":message});
        let answers = answers(&repo).await;
        let expected = answers
            .iter()
            .map(|answer| {
                let request = answer["request"].as_str().unwrap();
                let loads_file = request == "GET /requirements/req_shared" && kind != "assertion";
                if request.starts_with("GET ") && !loads_file {
                    json!({"request":request, "status":200, "error":null,
                        "freshness_error":format!("{file} line {second}: {message}")})
                } else {
                    json!({"request":request, "status":500, "error":refusal,
                        "freshness_error":null})
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(answers, expected, "{name}");
    }
}
