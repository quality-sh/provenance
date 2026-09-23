//! MCP schemas and result rendering for target-first authoring.

use super::authoring::{description, render_readable, Action, ActionError};
use provenance_core::NodeType;
use provenance_store::operations::catalog::Definition;
use rmcp::model::{CallToolResult, Content, Tool};
use serde_json::{json, Map, Value};

pub fn tools(host: &crate::StatementHost) -> Vec<Tool> {
    Action::ALL
        .into_iter()
        .filter_map(|action| tool(host, action))
        .collect()
}

fn tool(host: &crate::StatementHost, action: Action) -> Option<Tool> {
    let definitions = host.executable_target_definitions(action);
    if definitions.is_empty() {
        return None;
    }
    let input = input_schema(action, &definitions);
    let output = schema_union(
        definitions
            .iter()
            .map(|(_, definition)| definition.mcp_output_schema())
            .collect(),
        "anyOf",
    );
    let mut tool = Tool::new(
        action.as_str(),
        description(action),
        input
            .as_object()
            .expect("target action input schema is an object")
            .clone(),
    );
    tool.output_schema = Some(
        output
            .as_object()
            .expect("target action output schema is an object")
            .clone()
            .into(),
    );
    Some(tool)
}

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
            object["properties"]
                .as_object_mut()
                .expect("registered MCP properties are an object")
                .insert("target".into(), json!({"type":"string","minLength":1}));
            object["required"]
                .as_array_mut()
                .expect("registered MCP required is an array")
                .push(json!("target"));
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

pub async fn call(
    host: &crate::StatementHost,
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
        match crate::mcp::mcp_target_call(route.definition, &Value::Object(arguments), &target) {
            Ok(call) => call,
            Err(failure) => return crate::mcp::error(failure),
        };
    if !query.is_empty() {
        return action_error(&ActionError::InvalidOptions);
    }
    match host.invoke_target(&route, data, headers).await {
        Ok(value) => {
            let mut result = CallToolResult::structured(value.clone());
            result.content = vec![Content::text(render_readable(
                action, &target, route.kind, &value,
            ))];
            result
        }
        Err(failure) => crate::mcp::error(failure),
    }
}

fn action_error(error: &ActionError) -> CallToolResult {
    let kind = match error {
        ActionError::InvalidOptions | ActionError::KindSelection => "invalid_options",
        ActionError::NotFound => "not_found",
        ActionError::AmbiguousIdentity => "ambiguous_identity",
        ActionError::AccessDenied => "access_denied",
        ActionError::Operation(_) => "operation_failed",
    };
    let message = if matches!(error, ActionError::KindSelection) {
        "unsupported action options".to_owned()
    } else {
        error.to_string()
    };
    CallToolResult::structured_error(json!({"error":{"kind":kind,"message":message}}))
}
