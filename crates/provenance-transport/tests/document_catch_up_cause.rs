//! A document read on the HTTP route whose catch-up fails on a state line
//! that does not parse gives the file, the line and the parse message of
//! that line.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_macros::verifies;
use serde_json::{json, Value};
use support::records::Repository;
use support::resource_http::{call, host};

const REQUIREMENTS: &str = ".provenance/state/scopes/default/requirements/req.jsonl";

/// Puts a string where the line of `req_shared` holds a list of parents.
fn give_refines_a_string(repo: &Repository) {
    let path = repo.dir.path().join(REQUIREMENTS);
    let lines: Vec<String> = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|text| {
            let mut line: Value = serde_json::from_str(text).unwrap();
            if line["id"] == "req_shared" {
                line["refines"] = json!("req_parent");
            }
            format!("{line}\n")
        })
        .collect();
    std::fs::write(&path, lines.concat()).unwrap();
}

#[tokio::test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
async fn a_document_read_gives_the_line_that_stops_its_catch_up() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, false);
    let (status, first) = call(&host, "GET", "/requirements/req_shared/document", None).await;
    assert_eq!(status, 200, "{first}");
    give_refines_a_string(&repo);

    let (status, refused) = call(&host, "GET", "/requirements/req_shared/document", None).await;

    assert_eq!(status, 409);
    assert_eq!(
        refused,
        json!({
            "error": {
                "kind": "document_catch_up_failed",
                "cause": {
                    "kind": "invalid_state_file",
                    "file": "scopes/default/requirements/req.jsonl",
                    "line": 1,
                    "message": "invalid type: string \"req_parent\", expected a sequence"
                }
            },
            "meta": {}
        })
    );
}
