//! Each listed tool schema holds each identical definition once, and each reference in
//! it names a definition that the same schema holds.
#![cfg(feature = "test-fixture")]

#[allow(dead_code)]
mod support {
    #[path = "../../../provenance-transport/tests/support/check_port.rs"]
    pub mod check_port;
    #[path = "../../../provenance-transport/tests/support/records.rs"]
    pub mod records;
}

use provenance_transport::{
    fixture::{FixtureAccess, Target},
    StatementHost,
};
use rmcp::ServiceExt as _;
use serde_json::{json, Value};
use std::sync::Arc;
use support::{check_port::FixtureCheckPort, records::Repository};

fn full_host(repository: &Repository) -> StatementHost {
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
    StatementHost::with_fixture_access(access).with_check_port(Arc::new(FixtureCheckPort))
}

/// Every `$ref` value below `value`.
fn references(value: &Value, found: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get("$ref") {
                found.push(reference.clone());
            }
            object.values().for_each(|child| references(child, found));
        }
        Value::Array(values) => values.iter().for_each(|child| references(child, found)),
        _ => {}
    }
}

/// The problems of one tool schema: a definition held twice, or a reference to a
/// definition that the schema does not hold.
fn problems(tool: &str, schema: &Value) -> Vec<String> {
    let definitions = schema["$defs"].as_object().cloned().unwrap_or_default();
    let mut problems = Vec::new();
    let all = definitions.iter().collect::<Vec<_>>();
    for (index, (name, definition)) in all.iter().enumerate() {
        if let Some((other, _)) = all[..index]
            .iter()
            .find(|(_, earlier)| earlier == definition)
        {
            problems.push(format!("{tool}: {name} repeats {other}"));
        }
    }
    let mut found = Vec::new();
    references(schema, &mut found);
    for reference in found {
        let held = reference
            .strip_prefix("#/$defs/")
            .is_some_and(|name| definitions.contains_key(name));
        if !held {
            problems.push(format!("{tool}: {reference} does not resolve"));
        }
    }
    problems
}

#[tokio::test]
async fn each_tool_schema_holds_each_definition_once_and_resolves_each_reference() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let host = full_host(&repository);
    let (client_io, server_io) = tokio::io::duplex(1024 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    assert_eq!(tools.len(), 7);
    let mut found = Vec::new();
    for tool in &tools {
        found.extend(problems(&tool.name, &json!(tool.input_schema)));
        found.extend(problems(&tool.name, &json!(tool.output_schema)));
    }
    assert_eq!(found, Vec::<String>::new());

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

/// The check finds a repeated definition and a reference that does not resolve.
#[test]
fn the_check_finds_a_repeated_definition_and_a_missing_reference() {
    let schema = json!({
        "$defs": {"A": {"type": "string"}, "B": {"type": "string"}},
        "properties": {"a": {"$ref": "#/$defs/A"}, "c": {"$ref": "#/$defs/C"}}
    });
    assert_eq!(
        problems("tool", &schema),
        ["tool: B repeats A", "tool: #/$defs/C does not resolve"]
    );
}
