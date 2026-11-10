//! Characterization: the transport refusal rows as MCP error objects on the
//! base commit. Later changes to the store refusals keep these rows unchanged.
#![cfg(feature = "test-fixture")]

#[allow(dead_code)]
mod support {
    #[path = "../../../provenance-transport/tests/support/records.rs"]
    pub mod records;
}

use provenance_transport::{
    fixture::{FixtureAccess, Target},
    StatementHost,
};
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::{json, Value};
use support::records::Repository;

const REQUIREMENTS: &str = ".provenance/state/scopes/default/requirements/req.jsonl";

type Client = rmcp::service::RunningService<rmcp::RoleClient, ()>;

async fn call(client: &Client, name: &str, arguments: Value) -> Value {
    let result = client
        .call_tool(
            CallToolRequestParams::new(name.to_owned())
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    let mut content = result.structured_content.unwrap_or(Value::Null);
    if content["error"]["current_etag"].is_string() {
        content["error"]["current_etag"] = json!("<etag>");
    }
    json!({"is_error": result.is_error, "error": content["error"]})
}

fn create(id: &str, extra: &Value) -> Value {
    let mut input = json!({
        "actor":"reviewer", "id":id, "statement":"The system stores each record.",
        "status":"discovery", "depends_on":[], "supersedes":[]
    });
    for (key, value) in extra.as_object().unwrap() {
        input[key] = value.clone();
    }
    json!({"path":"requirements", "method":"post", "input": input})
}

async fn rows(client: &Client, repo: &Repository) -> Vec<Value> {
    let mut rows = Vec::new();
    for (name, arguments) in [
        (
            "create an existing requirement",
            create("req_shared", &json!({})),
        ),
        (
            "create with a missing dependency",
            create("req_new", &json!({"depends_on":["req_missing"]})),
        ),
        (
            "create with a blank actor",
            create("req_new", &json!({"actor":" "})),
        ),
        (
            "start a discussion with an empty body",
            json!({"path":"requirements/req_shared/discussions", "method":"post",
                "input":{"actor":"reviewer","declared_by":null,"role":"user","body":" "}}),
        ),
        (
            "read a missing requirement",
            json!({"path":"requirements/req_missing"}),
        ),
    ] {
        let mut answer = call(client, "api", arguments).await;
        answer["row"] = json!(name);
        rows.push(answer);
    }
    let mut answer = call(
        client,
        "change",
        json!({"action":"update", "target":"req_shared", "if_match":"\"stale\"",
            "actor":"reviewer", "description":"Edited."}),
    )
    .await;
    answer["row"] = json!("update with an old etag");
    rows.push(answer);

    let path = repo.dir.path().join(REQUIREMENTS);
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("{\"not\":\"a record\"}\n");
    std::fs::write(&path, text).unwrap();
    for (name, arguments) in [
        (
            "read a shard with a bad line",
            json!({"path":"requirements/req_shared"}),
        ),
        (
            "write to a shard with a bad line",
            create("req_new", &json!({})),
        ),
    ] {
        let mut answer = call(client, "api", arguments).await;
        answer["row"] = json!(name);
        rows.push(answer);
    }
    rows
}

#[tokio::test]
async fn each_refusal_keeps_its_mcp_error_object() {
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
    .unwrap()
    .allow_writes();
    let host = StatementHost::with_fixture_access(access);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let rows = rows(&client, &repo).await;
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
    let expected = json!([
        {
            "error": {
                "kind": "already_exists"
            },
            "is_error": true,
            "row": "create an existing requirement"
        },
        {
            "error": {
                "kind": "missing_reference"
            },
            "is_error": true,
            "row": "create with a missing dependency"
        },
        {
            "error": {
                "field": "request.actor",
                "kind": "invalid_input",
                "reason": "invalid_value"
            },
            "is_error": true,
            "row": "create with a blank actor"
        },
        {
            "error": {
                "kind": "empty_message_body"
            },
            "is_error": true,
            "row": "start a discussion with an empty body"
        },
        {
            "error": {
                "kind": "resource_not_found"
            },
            "is_error": true,
            "row": "read a missing requirement"
        },
        {
            "error": {
                "current_etag": "<etag>",
                "kind": "requirement_edit_conflict"
            },
            "is_error": true,
            "row": "update with an old etag"
        },
        {
            "error": {
                "file": "scopes/default/requirements/req.jsonl",
                "kind": "invalid_state_file",
                "line": 2,
                "message": "missing field `scope_id`"
            },
            "is_error": true,
            "row": "read a shard with a bad line"
        },
        {
            "error": {
                "file": "scopes/default/requirements/req.jsonl",
                "kind": "invalid_state_file",
                "line": 2,
                "message": "missing field `scope_id`"
            },
            "is_error": true,
            "row": "write to a shard with a bad line"
        }
    ]);
    assert_eq!(Value::Array(rows), expected);
}
