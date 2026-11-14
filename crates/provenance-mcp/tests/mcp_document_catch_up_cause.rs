//! A document read through the MCP `api` tool whose catch-up fails on a
//! state line that does not parse gives the file, the line and the parse
//! message of that line.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_macros::verifies;
use provenance_transport::{
    fixture::{FixtureAccess, Target},
    StatementHost,
};
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::{json, Value};
use support::records::Repository;

const REQUIREMENTS: &str = ".provenance/state/scopes/default/requirements/req.jsonl";

type Client = rmcp::service::RunningService<rmcp::RoleClient, ()>;

/// The `is_error` flag and the structured content of one document read.
async fn read_document(client: &Client) -> (Option<bool>, Value) {
    let arguments = json!({"path": "requirements/req_shared/document"});
    let result = client
        .call_tool(
            CallToolRequestParams::new("api".to_owned())
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    (
        result.is_error,
        result.structured_content.unwrap_or(Value::Null),
    )
}

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
    let access = FixtureAccess::new(
        vec![Target {
            id: "selected".into(),
            root: repo.dir.path().to_path_buf(),
        }],
        vec![("selected".into(), "default".into())],
        "fixture-secret",
        "fixture.test",
    )
    .unwrap();
    let host = StatementHost::with_fixture_access(access);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let (first_error, first) = read_document(&client).await;
    assert_ne!(first_error, Some(true), "{first}");
    give_refines_a_string(&repo);

    let (is_error, refused) = read_document(&client).await;

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
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
