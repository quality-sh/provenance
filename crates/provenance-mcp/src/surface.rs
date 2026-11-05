//! Shared MCP call and tool wiring.

use provenance_core::protocol::{failure::ErasedFailure, ResponseMeta};
use provenance_transport::{StatementHost, MAX_BODY_BYTES};
use rmcp::{
    model::{CallToolResult, Tool, ToolAnnotations},
    ErrorData,
};
use serde_json::{Map, Value};
use std::future::Future;

/// What a tool does to the graph, which sets its annotations.
#[derive(Clone, Copy)]
pub enum Effect {
    /// The tool only reads the repository graph.
    Reads,
    /// The tool writes. An update can replace field values, remove list entries, and
    /// archive a record, so a write can discard data. A repeated discuss starts a new
    /// Discussion. The notify action reaches a client thread outside the repository.
    Writes,
}

impl Effect {
    /// Read tools are read-only and closed-world; `change` is the one tool that writes.
    #[provenance_macros::rule("rule_mcp_tools_read_or_write")]
    fn annotations(self) -> ToolAnnotations {
        match self {
            Self::Reads => ToolAnnotations::from_raw(None, Some(true), None, None, Some(false)),
            Self::Writes => {
                ToolAnnotations::from_raw(None, Some(false), Some(true), Some(false), Some(true))
            }
        }
    }
}

#[derive(Clone, Copy)]
pub enum SchemaDeclaration {
    /// Keep the schema draft declaration in the advertised tool schema.
    Keep,
    /// Remove the schema draft declaration for clients that reject it.
    Remove,
}

/// Admit one MCP call, enforce its input limit, and keep its permit until completion.
pub async fn admitted_call<F, Fut>(
    host: &StatementHost,
    arguments: Option<Map<String, Value>>,
    invoke: F,
) -> Result<CallToolResult, ErrorData>
where
    F: FnOnce(Map<String, Value>) -> Fut,
    Fut: Future<Output = CallToolResult>,
{
    host.admitted(async {
        let arguments = arguments.unwrap_or_default();
        let encoded = serde_json::to_vec(&arguments)
            .map_err(|_| ErrorData::internal_error("Cannot encode input", None))?;
        if encoded.len() > MAX_BODY_BYTES {
            return Ok(failure_result(ErasedFailure::new(
                None,
                provenance_core::protocol::failure::OperationFailure::InvalidInput {
                    field: None,
                    reason: provenance_core::protocol::failure::InvalidInputReason::TooLarge,
                },
            )));
        }
        Ok(invoke(arguments).await)
    })
    .await
    .unwrap_or_else(|failure| Ok(failure_result(failure)))
}

/// Build one MCP tool from its JSON input and output schemas.
pub fn tool(
    name: &'static str,
    description: &'static str,
    effect: Effect,
    mut input: Value,
    output: Value,
    schema_declaration: SchemaDeclaration,
) -> Tool {
    if matches!(schema_declaration, SchemaDeclaration::Remove) {
        input.as_object_mut().map(|schema| schema.remove("$schema"));
    }
    let mut input = share_definitions(input);
    let mut output = share_definitions(output_schema(output, schema_declaration));
    crate::json_schema::to_2020_12(&mut input);
    crate::json_schema::to_2020_12(&mut output);
    let mut tool = Tool::new(
        name,
        description,
        input
            .as_object()
            .expect("MCP input schema is an object")
            .clone(),
    );
    tool.output_schema = Some(
        output
            .as_object()
            .expect("MCP output schema is an object")
            .clone()
            .into(),
    );
    tool.annotations = Some(effect.annotations());
    tool
}

fn output_schema(mut success: Value, schema_declaration: SchemaDeclaration) -> Value {
    let success = success
        .as_object_mut()
        .expect("MCP output schema is an object");
    let declaration = success.remove("$schema");
    let definitions = success.remove("$defs");
    let mut schema = Map::new();
    if matches!(schema_declaration, SchemaDeclaration::Keep) {
        if let Some(declaration) = declaration {
            schema.insert("$schema".to_owned(), declaration);
        }
    }
    if let Some(definitions) = definitions {
        schema.insert("$defs".to_owned(), definitions);
    }
    schema.insert("type".to_owned(), Value::String("object".to_owned()));
    // A result schema that is only a `oneOf` gives its branches to the root `oneOf`.
    let mut branches = match success.get("oneOf") {
        Some(Value::Array(branches)) if success.len() == 1 => branches.clone(),
        _ => vec![success.clone().into()],
    };
    branches.push(error_output_schema());
    schema.insert("oneOf".to_owned(), Value::Array(branches));
    Value::Object(schema)
}

/// Holds each identical `$defs` entry once: the first name in order stays, and each
/// reference to another name with the same schema points to it. A merge can make two
/// more entries identical, so it repeats until no two entries are the same.
fn share_definitions(mut schema: Value) -> Value {
    loop {
        let Some(Value::Object(definitions)) = schema.get("$defs") else {
            return schema;
        };
        let mut kept: Vec<(&String, &Value)> = Vec::new();
        let mut renamed = std::collections::BTreeMap::new();
        for (name, definition) in definitions {
            match kept.iter().find(|(_, candidate)| *candidate == definition) {
                Some((first, _)) => {
                    renamed.insert(format!("#/$defs/{name}"), format!("#/$defs/{first}"));
                }
                None => kept.push((name, definition)),
            }
        }
        if renamed.is_empty() {
            return schema;
        }
        let gone = renamed
            .keys()
            .map(|reference| reference.trim_start_matches("#/$defs/").to_owned())
            .collect::<Vec<_>>();
        rename_references(&mut schema, &renamed);
        let definitions = schema["$defs"]
            .as_object_mut()
            .expect("the definitions are an object");
        for name in gone {
            definitions.remove(&name);
        }
    }
}

fn rename_references(value: &mut Value, renamed: &std::collections::BTreeMap<String, String>) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref") {
                if let Some(target) = renamed.get(reference.as_str()) {
                    reference.clone_from(target);
                }
            }
            for child in object.values_mut() {
                rename_references(child, renamed);
            }
        }
        Value::Array(values) => {
            for child in values {
                rename_references(child, renamed);
            }
        }
        _ => {}
    }
}

fn error_output_schema() -> Value {
    serde_json::json!({
        "type":"object",
        "additionalProperties":false,
        "properties":{
            "error":{"type":"object"},
            "meta":{"type":"object"}
        },
        "required":["error", "meta"]
    })
}

/// Return one MCP error result with the shared response envelope.
pub fn error_result(error: Value, meta: Value) -> CallToolResult {
    CallToolResult::structured_error(Value::Object(Map::from_iter([
        ("error".to_owned(), error),
        ("meta".to_owned(), meta),
    ])))
}

/// Preserve the error and metadata from one erased operation failure.
pub fn failure_result(failure: ErasedFailure) -> CallToolResult {
    error_result(
        failure.error,
        serde_json::to_value(failure.meta).expect("response metadata is JSON"),
    )
}

/// Return an error result with empty response metadata.
pub fn detail_error(error: Value) -> CallToolResult {
    error_result(
        error,
        serde_json::to_value(ResponseMeta::default()).expect("response metadata is JSON"),
    )
}

/// Normalize an existing response envelope through the shared constructor.
pub fn envelope_error(envelope: &Value) -> CallToolResult {
    error_result(
        envelope["error"].clone(),
        envelope
            .get("meta")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    )
}
