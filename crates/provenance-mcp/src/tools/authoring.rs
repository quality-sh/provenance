//! The record actions of the change tool: their fields, and their calls by target.

use provenance_contract::{ArgumentPlace, Definition};
use provenance_core::protocol::failure::ErasedFailure;
use provenance_core::NodeType;
use provenance_macros::rule;
use provenance_porcelain::action::render_readable;
use provenance_porcelain::registry::Command;
use provenance_transport::porcelain::{
    target_call, Action, ActionError, ParentReplacement, ReplacementError, TargetRoute,
};
use provenance_transport::StatementHost;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{json, Map, Value};

/// The fields of one record action in the change tool, when the host can run it on
/// some record kind. Each argument of the operation is one property, as each is one
/// CLI flag. The record kind of the target selects the arguments that a call can give,
/// so a call that lacks a needed argument gets the schema of the kind.
#[rule("rule_mcp_one_tool_for_record_actions")]
pub fn action_schema(host: &StatementHost, action: Action) -> Option<Value> {
    let definitions = host.porcelain().executable_target_definitions(action);
    if definitions.is_empty() {
        return None;
    }
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
    if action == Action::Update {
        for field in Command::Action(action).declaration().input.fields() {
            let mut schema = json!({"type": "string", "minLength": 1});
            if let Some(doc) = field.doc {
                schema["description"] = json!(doc);
            }
            properties.insert(field.name.into(), schema);
        }
    }
    let mut definitions_by_name = Map::new();
    for (kind, definition) in &definitions {
        let schema = kind_schema(action, *kind, definition);
        for (name, property) in schema["properties"].as_object().into_iter().flatten() {
            match properties.get(name) {
                Some(held) if held != property => {
                    let mut branches = match held.get("anyOf") {
                        Some(Value::Array(branches)) => branches.clone(),
                        _ => vec![held.clone()],
                    };
                    if !branches.contains(property) {
                        branches.push(property.clone());
                    }
                    properties.insert(name.clone(), json!({"anyOf": branches}));
                }
                Some(_) => {}
                None => {
                    properties.insert(name.clone(), property.clone());
                }
            }
        }
        for (name, definition) in schema["$defs"].as_object().into_iter().flatten() {
            definitions_by_name
                .entry(name.clone())
                .or_insert_with(|| definition.clone());
        }
    }
    let mut schema = json!({"properties": properties, "required": required});
    if !definitions_by_name.is_empty() {
        schema["$defs"] = Value::Object(definitions_by_name);
    }
    Some(schema)
}

/// The arguments of one action on one record kind, with the definitions they refer
/// to. The target fills the record ID, so `id` is not an argument.
fn kind_schema(action: Action, kind: NodeType, definition: &Definition) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    let data_required = definition
        .request_schema()
        .and_then(|request| request.pointer("/properties/data/required"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for argument in definition.arguments().unwrap_or_default() {
        if argument.name == "id" {
            continue;
        }
        let (schema, needed) = match &argument.place {
            ArgumentPlace::Parameter(parameter) => (parameter.schema.clone(), parameter.required),
            ArgumentPlace::Body {
                field,
                schema,
                wrap_array,
            } => (
                schema.clone(),
                !wrap_array && data_required.iter().any(|name| name == field.as_str()),
            ),
        };
        if needed {
            required.push(json!(argument.name));
        }
        properties.insert(argument.name, schema);
    }
    let mut schema = json!({
        "type": "object",
        "additionalProperties": false,
        "properties": properties,
        "required": required,
        "description": format!("{} {}", action.as_str(), kind.as_str()),
    });
    if let Some(definitions) = definition
        .request_schema()
        .and_then(|request| request.get("$defs"))
    {
        schema["$defs"] = definitions.clone();
    }
    schema
}

/// The schema whose required list the call must meet now. A call without record fields
/// must give each required argument, and a refusal gives the schema of the kind. A call
/// with record fields goes to the operation, which names all the missing fields at once.
fn needed_now(mut schema: Value, definition: &Definition, arguments: &Map<String, Value>) -> Value {
    let declared = definition.arguments().unwrap_or_default();
    let body = |name: &str| {
        declared
            .iter()
            .any(|argument| argument.name == name && argument.body_field().is_some())
    };
    if arguments.keys().any(|name| body(name)) {
        if let Some(required) = schema["required"].as_array_mut() {
            required.retain(|name| name.as_str().is_some_and(|name| !body(name)));
        }
    }
    schema
}

/// The request arguments of one call: the body arguments go into `data`, and the
/// parameters stay. An alias of an array field gives one item of it.
fn request_arguments(
    definition: &Definition,
    arguments: Map<String, Value>,
) -> Result<Value, CallToolResult> {
    let declared = definition.arguments().unwrap_or_default();
    let mut data = Map::new();
    let mut request = Map::new();
    for (name, value) in arguments {
        let Some(argument) = declared.iter().find(|argument| argument.name == name) else {
            request.insert(name, value);
            continue;
        };
        let ArgumentPlace::Body {
            field, wrap_array, ..
        } = &argument.place
        else {
            request.insert(name, value);
            continue;
        };
        let value = if *wrap_array { json!([value]) } else { value };
        if data.insert(field.clone(), value).is_some() {
            let message = format!("{name} and {field} give the same field");
            return Err(refusal(&name, "invalid_value", &message));
        }
    }
    if !data.is_empty() {
        request.insert("data".into(), Value::Object(data));
    }
    Ok(Value::Object(request))
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
    let replace_parent = arguments.remove("replace_parent");
    let with_parent = arguments.remove("with_parent");
    let given = |name: &str| match name {
        "replace_parent" => replace_parent.is_some(),
        "with_parent" => with_parent.is_some(),
        name => arguments.contains_key(name),
    };
    let relations = Command::Action(action).declaration().input.relations();
    if let Some((relation, (field, _))) = relations
        .iter()
        .find_map(|relation| relation.broken_by(given).map(|broken| (relation, broken)))
    {
        return refusal(field, "invalid_value", &relation.message());
    }
    let schema = needed_now(
        kind_schema(action, route.kind, route.definition),
        route.definition,
        &arguments,
    );
    if let Some(refused) = field_refusal(action, route.kind, &schema, &arguments) {
        return refused;
    }
    let request = match request_arguments(route.definition, arguments) {
        Ok(request) => request,
        Err(refused) => return refused,
    };
    let written = match write(host, &route, &request, (replace_parent, with_parent)).await {
        Ok(written) => written,
        Err(refused) => return refused,
    };
    match written {
        (Ok(value), note) => {
            let mut structured = value.clone();
            if let Some(object) = structured.as_object_mut() {
                object.insert("type".into(), json!(route.kind.as_str()));
            }
            let mut result = CallToolResult::structured(structured);
            let readable = render_readable(action, &target, route.kind, &value);
            result.content = vec![ContentBlock::text(note.unwrap_or_default() + &readable)];
            result
        }
        (Err(failure), _) => crate::surface::failure_result(failure),
    }
}

/// The written record, or the failure of the write, and a note for the reader. With
/// both parent fields, the write replaces one Requirement parent.
async fn write(
    host: &StatementHost,
    route: &TargetRoute,
    request: &Value,
    (replace_parent, with_parent): (Option<Value>, Option<Value>),
) -> Result<(Result<Value, ErasedFailure>, Option<String>), CallToolResult> {
    let (data, query, headers) = target_call(route.definition, request, &route.target)
        .map_err(crate::surface::failure_result)?;
    if let Some(name) = query.keys().next() {
        let message = format!(
            "the {} action on a {} record does not take {name}",
            route.action.as_str(),
            route.kind.as_str()
        );
        return Err(refusal(name, "unknown_field", &message));
    }
    match (&replace_parent, &with_parent) {
        (Some(Value::String(old)), Some(Value::String(new))) => {
            let parents = ParentReplacement {
                replace_parent: old,
                with_parent: new,
            };
            match host.replace_parent(route, data, headers, parents).await {
                Ok(replaced) => {
                    let note = (!replaced.changed)
                        .then(|| format!("{new} replaced {old} already; nothing changed\n\n"));
                    Ok((Ok(replaced.record), note))
                }
                Err(ReplacementError::Failure(failure)) => Ok((Err(failure), None)),
                Err(ReplacementError::Invalid(message)) => {
                    Err(refusal("replace_parent", "invalid_value", &message))
                }
                Err(ReplacementError::Unchanged(message)) => Err(crate::surface::detail_error(
                    json!({"kind": "operation_failed", "message": message}),
                )),
            }
        }
        (None, None) => Ok((host.invoke_target(route, data, headers).await, None)),
        _ => Err(refusal(
            "replace_parent",
            "invalid_value",
            "the parents are record IDs",
        )),
    }
}

/// The refusal of a field that the record kind does not take, or of a field that it
/// needs and the call does not give. The refusal gives the fields of the kind.
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
    let mut schema = schema.clone();
    crate::json_schema::to_2020_12(&mut schema);
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
