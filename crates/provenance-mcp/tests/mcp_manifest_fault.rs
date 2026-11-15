//! The MCP server of a repository whose manifest stops parsing after the
//! server starts: each read and each write refuses with `invalid_state_file`,
//! the line of the manifest and the two procedures that correct the state.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_core::protocol::read_failure::STATE_CORRECTIONS;
use provenance_macros::verifies;
use provenance_transport::repository;
use serde_json::{json, Value};
use support::mcp::{call, Session};
use support::records::Repository;

#[tokio::test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
async fn a_manifest_that_stops_parsing_refuses_each_read_and_write_with_the_corrections() {
    let repo = Repository::new("The MCP server reads its manifest for each call.");
    let root = repository::utf8_root(repo.dir.path().to_path_buf(), "default").unwrap();
    let host = repository::open(&root, "default").unwrap();
    let session = Session::start(host).await;
    std::fs::write(repo.layout.manifest_path(), "{\"scopes\":\"default\"}\n").unwrap();

    let created = json!({"actor":"reviewer", "id":"req_new",
        "statement":"The system stores each record.", "status":"discovery", "depends_on":[],
        "supersedes":[]});
    let mut answers = Vec::new();
    for (tool, arguments) in [
        ("get", json!({"target":"req_shared"})),
        ("api", json!({"path":"requirements/req_shared", "method":"get"})),
        ("api", json!({"path":"requirements", "method":"post", "input":created})),
        (
            "change",
            json!({"action":"update", "target":"req_shared", "actor":"reviewer",
                "description":"Edited."}),
        ),
    ] {
        let answer = call(&session.client, tool, arguments).await;
        let content = answer.structured_content.unwrap_or(Value::Null);
        answers.push(json!({"tool": tool, "is_error": answer.is_error,
            "error": content["error"]}));
    }
    session.close().await;

    let refusal = json!({"kind":"invalid_state_file", "file":"manifest.json", "line":1,
        "message":"invalid type: string \"default\", expected a sequence",
        "hint":STATE_CORRECTIONS});
    assert_eq!(
        answers,
        ["get", "api", "api", "change"]
            .map(|tool| json!({"tool": tool, "is_error": true, "error": refusal}))
    );
}
