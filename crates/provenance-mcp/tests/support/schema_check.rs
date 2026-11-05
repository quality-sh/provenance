//! Tool calls checked against the listed schemas of their tool: the input schema
//! accepts each request that the server runs, and each result matches exactly one
//! branch of the output `oneOf` (a result or an error).

use rmcp::model::{CallToolRequestParams, CallToolResult};
use serde_json::Value;

pub type Client = rmcp::service::RunningService<rmcp::RoleClient, ()>;

/// Calls one listed tool and checks the request and the result against its schemas.
/// A tool that tools/list does not have fails the test.
pub async fn call(client: &Client, name: &str, arguments: Value) -> CallToolResult {
    let (input, output) = schemas(client, name).await;
    let result = client
        .call_tool(
            CallToolRequestParams::new(name.to_owned())
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    if result.is_error != Some(true) {
        assert!(compile(&input).is_valid(&arguments), "{name}: {arguments}");
    }
    let value = result.structured_content.as_ref().unwrap();
    assert_eq!(branches_matching(&output, value), 1, "{name}: {value}");
    result
}

/// The input and output schemas of one listed tool.
pub async fn schemas(client: &Client, name: &str) -> (Value, Value) {
    let tools = client.list_all_tools().await.unwrap();
    let tool = tools
        .iter()
        .find(|tool| tool.name == name)
        .unwrap_or_else(|| panic!("tools/list lacks {name}"));
    (
        Value::Object((*tool.input_schema).clone()),
        Value::Object((*tool.output_schema.clone().unwrap()).clone()),
    )
}

/// The number of root `oneOf` branches of `schema` that accept `value`, after the
/// whole schema accepts it. A schema without `oneOf` is one branch.
pub fn branches_matching(schema: &Value, value: &Value) -> usize {
    if !compile(schema).is_valid(value) {
        return 0;
    }
    let Some(branches) = schema["oneOf"].as_array() else {
        return 1;
    };
    branches
        .iter()
        .filter(|branch| {
            let mut branch = (*branch).clone();
            for keyword in ["$defs", "$schema"] {
                if let Some(value) = schema.get(keyword) {
                    branch[keyword] = value.clone();
                }
            }
            compile(&branch).is_valid(value)
        })
        .count()
}

/// Read a schema as a JSON Schema 2020-12 document, the dialect that MCP gives a schema
/// without `$schema`. Each format is an assertion, and a format that 2020-12 does not
/// define makes the schema invalid.
pub fn compile(schema: &Value) -> jsonschema::Validator {
    if let Some(dialect) = schema.get("$schema") {
        assert_eq!(dialect, "https://json-schema.org/draft/2020-12/schema");
    }
    jsonschema::draft202012::options()
        .should_validate_formats(true)
        .should_ignore_unknown_formats(false)
        .build(schema)
        .unwrap_or_else(|error| panic!("{error}: {schema}"))
}
