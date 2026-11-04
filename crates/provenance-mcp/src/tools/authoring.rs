//! MCP schemas and result rendering for target-first authoring.

use provenance_contract::Definition;
use provenance_core::NodeType;
use provenance_macros::rule;
use provenance_porcelain::action::render_readable;
use provenance_porcelain::registry::Command;
use provenance_transport::porcelain::{target_call, Action, ActionError};
use provenance_transport::StatementHost;
use rmcp::model::{CallToolResult, ContentBlock, Tool};
use serde_json::{json, Map, Value};

/// The change tool: one branch for each record action that the host can run on some
/// record kind. The action argument selects the branch, so a valid request matches
/// exactly one branch; inside it, the record kind of the target selects the fields.
#[rule("rule_mcp_one_tool_for_record_actions")]
pub fn change_tool(host: &StatementHost) -> Option<Tool> {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for action in Action::RECORD {
        let definitions = host.porcelain().executable_target_definitions(action);
        if definitions.is_empty() {
            continue;
        }
        inputs.push(input_schema(action, &definitions));
        for (_, definition) in &definitions {
            let output = definition.mcp_output_schema();
            if !outputs.contains(&output) {
                outputs.push(output);
            }
        }
    }
    if inputs.is_empty() {
        return None;
    }
    let declaration = Command::Change.declaration();
    Some(crate::surface::tool(
        declaration.name,
        declaration.description,
        branch_union(inputs, "Action", "oneOf"),
        // Results of different record kinds overlap: a Source record has every field
        // that a Domain record needs. The tool output keeps the choice between a
        // result and an error as `oneOf`.
        branch_union(outputs, "Output", "anyOf"),
        crate::surface::SchemaDeclaration::Keep,
    ))
}

/// A union of the branches, with each branch's definitions held once at the root.
fn branch_union(mut branches: Vec<Value>, prefix: &str, keyword: &str) -> Value {
    if branches.len() == 1 {
        return branches.pop().expect("one branch");
    }
    let mut definitions = Map::new();
    for (index, branch) in branches.iter_mut().enumerate() {
        namespace_definitions(branch, &format!("{prefix}{index}"), &mut definitions);
    }
    let mut schema = json!({ keyword: branches });
    if !definitions.is_empty() {
        schema["$defs"] = Value::Object(definitions);
    }
    schema
}

/// The input of one record action: one variant for each record kind, each with the
/// action argument.
fn input_schema(action: Action, definitions: &[(NodeType, &'static Definition)]) -> Value {
    let variants = definitions
        .iter()
        .map(|(kind, definition)| {
            let mut schema = definition.mcp_input_schema();
            let object = schema
                .as_object_mut()
                .expect("registered MCP input schema is an object");
            if action == Action::Create {
                {
                    let properties = object["properties"]
                        .as_object_mut()
                        .expect("registered MCP properties are an object");
                    let data = properties["data"]
                        .as_object_mut()
                        .expect("create data schema is an object");
                    data["properties"]
                        .as_object_mut()
                        .expect("create properties are an object")
                        .remove("id");
                    data["required"]
                        .as_array_mut()
                        .expect("create required is an array")
                        .retain(|field| field != "id");
                    properties.insert("type".into(), json!({"const":kind.as_str()}));
                }
                object["required"]
                    .as_array_mut()
                    .expect("registered MCP required is an array")
                    .push(json!("type"));
            } else {
                object["properties"]
                    .as_object_mut()
                    .expect("registered MCP properties are an object")
                    .remove("id");
                object["required"]
                    .as_array_mut()
                    .expect("registered MCP required is an array")
                    .retain(|field| field != "id");
            }
            let properties = object["properties"]
                .as_object_mut()
                .expect("registered MCP properties are an object");
            properties.insert("action".into(), json!({"const":action.as_str()}));
            properties.insert("target".into(), json!({"type":"string","minLength":1}));
            let required = object["required"]
                .as_array_mut()
                .expect("registered MCP required is an array");
            required.push(json!("action"));
            required.push(json!("target"));
            remove_empty_data(object);
            schema
        })
        .collect();
    schema_union(
        variants,
        if action == Action::Create {
            "oneOf"
        } else {
            "anyOf"
        },
    )
}

fn remove_empty_data(schema: &mut Map<String, Value>) {
    let empty = schema["properties"]["data"]["properties"]
        .as_object()
        .is_some_and(Map::is_empty)
        && schema["properties"]["data"]["required"]
            .as_array()
            .is_some_and(Vec::is_empty);
    if empty {
        schema["properties"]
            .as_object_mut()
            .expect("properties are an object")
            .remove("data");
        schema["required"]
            .as_array_mut()
            .expect("required is an array")
            .retain(|field| field != "data");
    }
}

fn schema_union(mut variants: Vec<Value>, keyword: &str) -> Value {
    let mut definitions = Map::new();
    for (index, variant) in variants.iter_mut().enumerate() {
        namespace_definitions(variant, &format!("Target{index}"), &mut definitions);
    }
    let mut schema = if variants.len() == 1 {
        variants.pop().expect("one schema")
    } else {
        Value::Object(Map::from_iter([(
            keyword.to_owned(),
            Value::Array(variants),
        )]))
    };
    if !definitions.is_empty() {
        schema["$defs"] = Value::Object(definitions);
    }
    schema
}

fn namespace_definitions(value: &mut Value, prefix: &str, merged: &mut Map<String, Value>) {
    if let Some(Value::Object(definitions)) = value
        .as_object_mut()
        .and_then(|object| object.remove("$defs"))
    {
        for (name, mut schema) in definitions {
            rewrite_references(&mut schema, prefix);
            merged.insert(format!("{prefix}{name}"), schema);
        }
    }
    rewrite_references(value, prefix);
}

fn rewrite_references(value: &mut Value, prefix: &str) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref") {
                if let Some(name) = reference.strip_prefix("#/$defs/") {
                    *reference = format!("#/$defs/{prefix}{name}");
                }
            }
            for child in object.values_mut() {
                rewrite_references(child, prefix);
            }
        }
        Value::Array(values) => {
            for child in values {
                rewrite_references(child, prefix);
            }
        }
        _ => {}
    }
}

/// Runs the record action that the action argument of the change tool names.
pub async fn call_change(
    host: &StatementHost,
    mut arguments: Map<String, Value>,
) -> CallToolResult {
    let action = arguments
        .remove("action")
        .and_then(|value| value.as_str().and_then(Action::parse))
        .filter(|action| Action::RECORD.contains(action));
    match action {
        Some(action) => call(host, action, arguments).await,
        None => action_error(&ActionError::InvalidOptions),
    }
}

/// Reads the target ID from the structured MCP arguments.
#[rule("rule_porcelain_mcp_target_argument")]
pub async fn call(
    host: &StatementHost,
    action: Action,
    mut arguments: Map<String, Value>,
) -> CallToolResult {
    let Some(target) = arguments
        .remove("target")
        .and_then(|value| value.as_str().map(str::to_owned))
        .filter(|target| !target.is_empty())
    else {
        return action_error(&ActionError::InvalidOptions);
    };
    let kind = match arguments.remove("type") {
        None => None,
        Some(Value::String(value)) => match NodeType::parse(&value) {
            Ok(kind) => Some(kind),
            Err(_) => return action_error(&ActionError::InvalidOptions),
        },
        Some(_) => return action_error(&ActionError::InvalidOptions),
    };
    let route = match host.target_route(action, &target, kind).await {
        Ok(route) => route,
        Err(error) => return action_error(&error),
    };
    let (data, query, headers) =
        match target_call(route.definition, &Value::Object(arguments), &target) {
            Ok(call) => call,
            Err(failure) => return crate::surface::failure_result(failure),
        };
    if !query.is_empty() {
        return action_error(&ActionError::InvalidOptions);
    }
    match host.invoke_target(&route, data, headers).await {
        Ok(value) => {
            let mut result = CallToolResult::structured(value.clone());
            result.content = vec![ContentBlock::text(render_readable(
                action, &target, route.kind, &value,
            ))];
            result
        }
        Err(failure) => crate::surface::failure_result(failure),
    }
}

fn action_error(error: &ActionError) -> CallToolResult {
    let kind = match error {
        ActionError::InvalidOptions | ActionError::KindSelection => "invalid_options",
        ActionError::NotFound => "not_found",
        ActionError::AmbiguousIdentity => "ambiguous_identity",
        ActionError::AccessDenied => "access_denied",
        ActionError::Operation(_) | ActionError::OperationDetail { .. } => "operation_failed",
    };
    let message = if matches!(error, ActionError::KindSelection) {
        "unsupported action options".to_owned()
    } else {
        error.to_string()
    };
    crate::surface::detail_error(json!({"kind":kind,"message":message}))
}
