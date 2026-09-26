#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
}

use provenance_porcelain::check::{
    BindingContext, BindingPolicy, Category, CategoryRun, CheckPort, PortFuture, Refusal,
};
use provenance_transport::{
    fixture::{FixtureAccess, Target},
    StatementHost,
};
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::{json, Value};
use std::sync::Arc;
use support::records::Repository;

struct FixtureCheckPort;

impl CheckPort for FixtureCheckPort {
    fn run<'a>(&'a self, category: Category, _: Option<&'a str>) -> PortFuture<'a> {
        Box::pin(async move {
            Ok(match category {
                Category::Graph => CategoryRun::Graph {
                    findings: Vec::new(),
                    refusal: Refusal::None,
                },
                Category::Statements => CategoryRun::Statements {
                    findings: Vec::new(),
                    context: None,
                    refusal: Refusal::None,
                },
                Category::Bindings => CategoryRun::Bindings {
                    findings: Vec::new(),
                    context: BindingContext {
                        policy: BindingPolicy::Warning,
                    },
                    refusal: Refusal::None,
                },
            })
        })
    }
}

async fn call(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    name: &str,
    arguments: Value,
) -> rmcp::model::CallToolResult {
    client
        .call_tool(
            CallToolRequestParams::new(name.to_owned())
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap()
}

fn assert_error_envelope(name: &str, result: &rmcp::model::CallToolResult) {
    assert_eq!(result.is_error, Some(true), "{name}: {result:?}");
    let envelope = result
        .structured_content
        .as_ref()
        .and_then(Value::as_object)
        .unwrap();
    let mut fields = envelope.keys().map(String::as_str).collect::<Vec<_>>();
    fields.sort_unstable();
    assert_eq!(fields, ["error", "meta"], "{name}: {envelope:?}");
    assert!(envelope["error"].is_object(), "{name}: {envelope:?}");
    assert!(envelope["meta"].is_object(), "{name}: {envelope:?}");
}

fn assert_matches_output_schema(
    family: &str,
    tool: &rmcp::model::Tool,
    result: &rmcp::model::CallToolResult,
) {
    let schema = json!(tool.output_schema.as_ref().unwrap());
    let validator = jsonschema::JSONSchema::compile(&schema).unwrap();
    let structured = result.structured_content.as_ref().unwrap();
    assert!(
        validator.is_valid(structured),
        "{family}: {structured:?} does not match {schema:?}"
    );
}

#[tokio::test]
async fn every_mcp_tool_family_uses_the_error_envelope() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let access = FixtureAccess::new(
        vec![Target {
            id: "selected".into(),
            root: repository.dir.path().to_path_buf(),
        }],
        vec![("selected".into(), "default".into())],
        "fixture-secret",
        "fixture.test",
    )
    .unwrap()
    .allow_writes();
    let host =
        StatementHost::with_fixture_access(access).with_check_port(Arc::new(FixtureCheckPort));
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    let get_schema = tools
        .iter()
        .find(|tool| tool.name == "get")
        .and_then(|tool| tool.output_schema.as_ref())
        .unwrap();
    assert!(get_schema["$schema"].is_string());
    assert!(get_schema["$defs"].is_object());
    assert!(get_schema["oneOf"][0].get("$defs").is_none());
    let search_schema = tools
        .iter()
        .find(|tool| tool.name == "search")
        .and_then(|tool| tool.output_schema.as_ref())
        .unwrap();
    assert!(search_schema.get("$schema").is_none());

    for (family, tool, arguments) in [
        ("catalog", "get-source", json!({})),
        ("get", "get", json!({"unknown":true})),
        ("search", "search", json!({"unknown":true})),
        ("api", "api", json!({"method":"post"})),
        ("authoring", "create", json!({})),
        ("discussion", "discussion", json!({})),
        ("check", "check", json!({"unknown":true})),
    ] {
        let result = call(&client, tool, arguments).await;
        assert_error_envelope(family, &result);
        let declaration = tools
            .iter()
            .find(|candidate| candidate.name == tool)
            .unwrap();
        assert_matches_output_schema(family, declaration, &result);
    }

    for (family, tool, arguments) in [
        ("catalog", "get-source", json!({"id":"source_shared"})),
        ("get", "get", json!({"target":"req_shared"})),
        (
            "search",
            "search",
            json!({"text":"no-record-has-this-text"}),
        ),
        ("api", "api", json!({})),
        (
            "authoring",
            "create",
            json!({
                "target":"source_schema_success",
                "type":"source",
                "data":{
                    "name":"Schema success",
                    "source_type":"policy",
                    "url":"https://example.test/schema-success",
                    "supersedes":[]
                }
            }),
        ),
        ("discussion", "discussions", json!({})),
        ("check", "check", json!({"categories":["graph"]})),
    ] {
        let result = call(&client, tool, arguments).await;
        assert_ne!(result.is_error, Some(true), "{family}: {result:?}");
        let declaration = tools
            .iter()
            .find(|candidate| candidate.name == tool)
            .unwrap();
        assert_matches_output_schema(family, declaration, &result);
    }

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
