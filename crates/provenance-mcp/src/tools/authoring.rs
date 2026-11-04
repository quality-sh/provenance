//! The record actions of the change tool: their fields, and their calls by target.

use provenance_contract::Definition;
use provenance_core::NodeType;
use provenance_macros::rule;
use provenance_porcelain::action::render_readable;
use provenance_transport::porcelain::{target_call, Action, ActionError};
use provenance_transport::StatementHost;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{json, Map, Value};

/// The fields of one record action in the change tool, when the host can run it on
/// some record kind. The record kind selects the fields of `data`, so the tool input
/// gives `data` as an object, and a call without `data` gets the schema of the kind.
#[rule("rule_mcp_one_tool_for_record_actions")]
pub fn action_schema(host: &StatementHost, action: Action) -> Option<Value> {
    let definitions = host.porcelain().executable_target_definitions(action);
    if definitions.is_empty() {
        return None;
    }
    let kinds = definitions
        .iter()
        .map(|(kind, definition)| kind_schema(action, *kind, definition))
        .collect::<Vec<_>>();
    let mut properties = Map::new();
    properties.insert(
        "target".into(),
        json!({"type": "string", "minLength": 1, "description": "The record ID."}),
    );
    let mut required = vec![json!("target")];
    if action == Action::Create {
        let names = definitions
            .iter()
            .map(|(kind, _)| kind.as_str())
            .collect::<Vec<_>>();
        properties.insert(
            "type".into(),
            json!({"type": "string", "enum": names, "description": "The record kind."}),
        );
        required.push(json!("type"));
    }
    for kind in &kinds {
        for (name, schema) in kind["properties"].as_object().into_iter().flatten() {
            let field = if name == "data" {
                json!({
                    "type": "object",
                    "description": "The record fields. Omit data to get their schema \
                                    for the action and record kind."
                })
            } else {
                schema.clone()
            };
            properties.entry(name.clone()).or_insert(field);
        }
    }
    // The record kind decides if `data` is needed: a call without it gets the schema.
    Some(json!({"properties": properties, "required": required}))
}

/// The fields of one action on one record kind, with the definitions they refer to.
fn kind_schema(action: Action, kind: NodeType, definition: &Definition) -> Value {
    let mut schema = definition.mcp_input_schema();
    let object = schema
        .as_object_mut()
        .expect("registered MCP input schema is an object");
    let properties = object["properties"]
        .as_object_mut()
        .expect("registered MCP properties are an object");
    if action == Action::Create {
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
    } else {
        properties.remove("id");
        object["required"]
            .as_array_mut()
            .expect("registered MCP required is an array")
            .retain(|field| field != "id");
    }
    remove_empty_data(object);
    object.insert(
        "description".into(),
        json!(format!("{} {}", action.as_str(), kind.as_str())),
    );
    schema
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

/// Runs one record action. The tool input has checked that the call gives the fields
/// of the action; the record kind of the target then selects the fields of `data`.
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
        return refusal("target", "invalid_value", "the target is a record ID");
    };
    let kind = match arguments.remove("type") {
        None => None,
        Some(Value::String(value)) => match NodeType::parse(&value) {
            Ok(kind) => Some(kind),
            Err(_) => return wrong_kind(host, action, "type"),
        },
        Some(_) => return wrong_kind(host, action, "type"),
    };
    let route = match host.target_route(action, &target, kind).await {
        Ok(route) => route,
        Err(ActionError::InvalidOptions) if action == Action::Create => {
            return wrong_kind(host, action, "type")
        }
        Err(ActionError::InvalidOptions) => return wrong_kind(host, action, "target"),
        Err(error) => return action_error(&error),
    };
    let schema = kind_schema(action, route.kind, route.definition);
    if let Some(refused) = field_refusal(action, route.kind, &schema, &arguments) {
        return refused;
    }
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

/// The refusal of a field that the record kind does not take, or of a field that it
/// needs and the call does not give. The refusal gives the fields of the kind.
#[rule("rule_mcp_refusal_names_field")]
fn field_refusal(
    action: Action,
    kind: NodeType,
    schema: &Value,
    arguments: &Map<String, Value>,
) -> Option<CallToolResult> {
    let properties = schema["properties"].as_object()?;
    let on = format!(
        "the {} action on a {} record",
        action.as_str(),
        kind.as_str()
    );
    let (field, reason, message) = if let Some(name) = arguments
        .keys()
        .find(|name| !properties.contains_key(*name))
    {
        (
            name.clone(),
            "unknown_field",
            format!("{on} does not take {name}"),
        )
    } else {
        let required = schema["required"].as_array()?;
        let name = required
            .iter()
            .filter_map(Value::as_str)
            .find(|name| arguments.get(*name).is_none_or(Value::is_null))?;
        (name.to_owned(), "required", format!("{on} needs {name}"))
    };
    Some(crate::surface::detail_error(json!({
        "kind": "invalid_input",
        "field": field,
        "reason": reason,
        "message": message,
        "schema": schema,
    })))
}

/// The refusal of a target or type whose record kind the action does not take.
fn wrong_kind(host: &StatementHost, action: Action, field: &str) -> CallToolResult {
    let kinds = host
        .porcelain()
        .executable_target_definitions(action)
        .iter()
        .map(|(kind, _)| kind.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let message = format!(
        "the {} action takes a record of kind: {kinds}",
        action.as_str()
    );
    refusal(field, "invalid_value", &message)
}

fn refusal(field: &str, reason: &str, message: &str) -> CallToolResult {
    crate::surface::detail_error(json!({
        "kind": "invalid_input",
        "field": field,
        "reason": reason,
        "message": message,
    }))
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
