//! Target-first routing over registered record operations.

use axum::http::HeaderMap;
use provenance_core::{protocol::RecordResolution, NodeType};
use provenance_macros::rule;
use provenance_store::operations::catalog::{self, Definition};
use rmcp::model::{CallToolResult, Content, Tool};
use serde_json::{json, Map, Value};
use std::{collections::BTreeMap, fmt::Display};

use super::get_port;

/// One target-first mutation name shared by CLI and MCP.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Create,
    Update,
    Answer,
    Claim,
    Release,
    Submit,
}

impl Action {
    pub const ALL: [Self; 6] = [
        Self::Create,
        Self::Update,
        Self::Answer,
        Self::Claim,
        Self::Release,
        Self::Submit,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Answer => "answer",
            Self::Claim => "claim",
            Self::Release => "release",
            Self::Submit => "submit",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|action| action.as_str() == value)
    }

    const fn supports(self, kind: NodeType) -> bool {
        match self {
            Self::Create | Self::Update => true,
            Self::Answer => matches!(kind, NodeType::Question),
            Self::Claim | Self::Release => matches!(kind, NodeType::Topic),
            Self::Submit => matches!(kind, NodeType::Requirement),
        }
    }
}

/// A registered operation selected for one target-first action.
pub struct TargetRoute {
    pub action: Action,
    pub target: String,
    pub kind: NodeType,
    pub definition: &'static Definition,
    pub path: String,
}

/// A target-first action that cannot be routed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActionError {
    InvalidOptions,
    NotFound,
    AmbiguousIdentity,
    Operation(String),
}

impl Display for ActionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOptions => formatter.write_str("unsupported action options"),
            Self::NotFound => formatter.write_str("record does not exist"),
            Self::AmbiguousIdentity => formatter.write_str("record ID is not unique"),
            Self::Operation(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ActionError {}

impl crate::StatementHost {
    /// Resolve the target kind and select its registered operation.
    #[rule("rule_porcelain_create_names_new_record")]
    #[rule("rule_porcelain_existing_action_infers_kind")]
    #[rule("rule_porcelain_named_domain_actions")]
    pub async fn target_route(
        &self,
        action: Action,
        target: &str,
        create_kind: Option<NodeType>,
    ) -> Result<TargetRoute, ActionError> {
        if target.is_empty() || (action == Action::Create) != create_kind.is_some() {
            return Err(ActionError::InvalidOptions);
        }
        let kind = if let Some(kind) = create_kind {
            kind
        } else {
            let response = get_port::resolve(self, target)
                .await
                .map_err(ActionError::Operation)?;
            match response {
                RecordResolution::Found(record) => record.node_type(),
                RecordResolution::Missing => return Err(ActionError::NotFound),
                RecordResolution::Ambiguous => return Err(ActionError::AmbiguousIdentity),
            }
        };
        let definition = definition(action, kind).ok_or(ActionError::InvalidOptions)?;
        let path = definition.path.replace("{id}", target);
        Ok(TargetRoute {
            action,
            target: target.to_owned(),
            kind,
            definition,
            path,
        })
    }

    /// Invoke the selected registered operation without a network loopback.
    pub async fn invoke_target(
        &self,
        route: &TargetRoute,
        mut data: Value,
        headers: HeaderMap,
    ) -> Result<Value, provenance_core::protocol::failure::ErasedFailure> {
        if route.action == Action::Create {
            let object = data
                .as_object_mut()
                .ok_or_else(|| crate::routing::invalid(None))?;
            if object.insert("id".into(), json!(route.target)).is_some() {
                return Err(crate::routing::invalid(Some("id")));
            }
        }
        let method = match route.definition.method {
            catalog::HttpMethod::Get => axum::http::Method::GET,
            catalog::HttpMethod::Post => axum::http::Method::POST,
            catalog::HttpMethod::Patch => axum::http::Method::PATCH,
        };
        self.invoke_resource(method, &route.path, data, BTreeMap::new(), headers)
            .await
    }
}

fn definition(action: Action, kind: NodeType) -> Option<&'static Definition> {
    if !action.supports(kind) {
        return None;
    }
    let name = if action == Action::Submit {
        "submit-requirement-review".to_owned()
    } else {
        format!("{}-{}", action.as_str(), kind.as_str())
    };
    catalog::definitions()
        .iter()
        .find(|definition| definition.name == name)
}

pub(crate) fn tools(host: &crate::StatementHost) -> Vec<Tool> {
    Action::ALL
        .into_iter()
        .filter_map(|action| tool(host, action))
        .collect()
}

fn tool(host: &crate::StatementHost, action: Action) -> Option<Tool> {
    if action != Action::Create && !get_port::is_resolver_available(host) {
        return None;
    }
    let definitions = NodeType::ALL
        .into_iter()
        .filter_map(|kind| definition(action, kind).map(|definition| (kind, definition)))
        .filter(|(_, definition)| host.advertises(definition.name))
        .collect::<Vec<_>>();
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
        input.as_object()
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

const fn description(action: Action) -> &'static str {
    match action {
        Action::Create => "Create the target ID as an explicit record type.",
        Action::Update => "Update the existing target while preserving omitted fields.",
        Action::Answer => "Answer the target Question.",
        Action::Claim => "Claim the target Topic.",
        Action::Release => "Release the target Topic claim.",
        Action::Submit => "Submit the target Requirement for review.",
    }
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
                .insert(
                    "target".into(),
                    json!({"type":"string","minLength":1}),
                );
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
        Value::Object(Map::from_iter([(keyword.to_owned(), Value::Array(variants))]))
    };
    if !definitions.is_empty() {
        schema["$defs"] = Value::Object(definitions);
    }
    schema
}

fn namespace_definitions(value: &mut Value, prefix: &str, merged: &mut Map<String, Value>) {
    if let Some(definitions) = value.as_object_mut().and_then(|object| object.remove("$defs")) {
        if let Value::Object(definitions) = definitions {
            for (name, mut schema) in definitions {
                rewrite_references(&mut schema, prefix);
                merged.insert(format!("{prefix}{name}"), schema);
            }
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

pub(crate) async fn call(
    host: &crate::StatementHost,
    action: Action,
    mut arguments: Map<String, Value>,
) -> CallToolResult {
    let Some(target) = arguments
        .remove("target")
        .and_then(|value| value.as_str().map(str::to_owned))
        .filter(|target| !target.is_empty())
    else {
        return action_error(ActionError::InvalidOptions);
    };
    let kind = arguments
        .remove("type")
        .and_then(|value| value.as_str().and_then(|value| NodeType::parse(value).ok()));
    let route = match host.target_route(action, &target, kind).await {
        Ok(route) => route,
        Err(error) => return action_error(error),
    };
    if action == Action::Create {
        let Some(data) = arguments.get_mut("data").and_then(Value::as_object_mut) else {
            return action_error(ActionError::InvalidOptions);
        };
        if data.insert("id".into(), json!(target)).is_some() {
            return action_error(ActionError::InvalidOptions);
        }
    } else {
        arguments.insert("id".into(), json!(target));
    }
    let (matched, data, query, headers) =
        match crate::mcp::mcp_call(route.definition, &Value::Object(arguments)) {
            Ok(call) => call,
            Err(failure) => return crate::mcp::error(failure),
        };
    match crate::routing::invoke(host, &matched, data, query, &headers).await {
        Ok((value, _)) => {
            let value = value.into_value();
            let mut result = CallToolResult::structured(value.clone());
            result.content = vec![Content::text(render_readable(
                action, &target, route.kind, &value,
            ))];
            result
        }
        Err(failure) => crate::mcp::error(failure),
    }
}

fn action_error(error: ActionError) -> CallToolResult {
    let kind = match error {
        ActionError::InvalidOptions => "invalid_options",
        ActionError::NotFound => "not_found",
        ActionError::AmbiguousIdentity => "ambiguous_identity",
        ActionError::Operation(_) => "operation_failed",
    };
    CallToolResult::structured_error(json!({
        "error":{"kind":kind,"message":error.to_string()}
    }))
}

/// Render one catalog result for a reader while preserving the structured value.
pub fn render_readable(action: Action, target: &str, kind: NodeType, value: &Value) -> String {
    format!(
        "{} {} {}\n\n{}",
        action.as_str(),
        kind.as_str(),
        target,
        serde_json::to_string_pretty(value.get("data").unwrap_or(value))
            .expect("registered output is JSON")
    )
}
