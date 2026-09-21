//! MCP-owned bindings for shared Porcelain capabilities.

use provenance_porcelain::check::{Category, CheckInput};
use provenance_porcelain::get::{GetInput, ReadError, View};
use rmcp::model::{CallToolResult, Content};
use serde::Deserialize;
use serde_json::{Map, Value};

pub(crate) mod authoring;
mod get_port;
pub(super) use authoring::{call as call_authoring, tools as authoring_tools};
pub use authoring::{render_readable as render_action_readable, Action, ActionError, TargetRoute};
pub use get_port::HostGetPort;

/// MCP input for the `check` Porcelain action.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CheckArguments {
    #[serde(default)]
    pub categories: Vec<Category>,
}

impl CheckArguments {
    /// Translate MCP selectors into one shared semantic request.
    pub fn into_check_input(self) -> CheckInput {
        CheckInput::new(self.categories)
    }
}

pub(crate) fn get_is_available(host: &crate::StatementHost) -> bool {
    get_port::is_available(host)
}

pub(crate) fn get_tool() -> rmcp::model::Tool {
    let record_kinds = provenance_core::NodeType::ALL.map(provenance_core::NodeType::as_str);
    let schema = serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["target"],
        "properties": {
            "target": {"type": "string", "minLength": 1},
            "view": {"type": "string", "enum": ["record", "children", "grounding", "impact"], "default": "record"},
            "max_depth": {"type": "integer", "minimum": 1},
            "returned_kinds": {"type": "array", "items": {"type": "string", "enum": record_kinds}},
            "limit": {"type": "integer", "minimum": 1}
        }
    });
    let mut tool = rmcp::model::Tool::new(
        "get",
        "Read one repository record by its repository-local ID.",
        schema.as_object().expect("get schema is an object").clone(),
    );
    tool.output_schema = Some(get_output_schema().into());
    tool
}

pub(crate) fn check_tool() -> rmcp::model::Tool {
    let schema = serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "categories": {
                "type": "array",
                "items": {"type": "string", "enum": ["graph", "statements", "bindings"]},
                "uniqueItems": true
            }
        }
    });
    let mut tool = rmcp::model::Tool::new(
        "check",
        "Check graph validity, statement quality, and binding coverage.",
        schema
            .as_object()
            .expect("check schema is an object")
            .clone(),
    );
    tool.output_schema = Some(check_output_schema().into());
    tool
}

fn get_output_schema() -> Map<String, Value> {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["record", "view", "related", "detail", "bounds"],
        "properties": {
            "record": {"$ref": "#/$defs/record"},
            "view": {"type": "string", "enum": ["record", "children", "grounding", "impact"]},
            "related": {"type": "array", "items": {"$ref": "#/$defs/record"}},
            "detail": {}, "bounds": {"anyOf": [{"type": "null"}, {"$ref": "#/$defs/bounds"}]},
            "record_metadata": {}, "view_metadata": {}
        },
        "$defs": {
            "record": {"type": "object", "additionalProperties": false,
                "required": ["id", "kind", "value"], "properties": {
                    "id": {"type": "string"}, "kind": {"type": "string"}, "value": {},
                    "depth": {"type": "integer", "minimum": 1}
                }},
            "bounds": {"type": "object", "additionalProperties": false,
                "required": ["limit", "max_depth", "has_more", "continuation", "truncated"],
                "properties": {"limit": {"type": "integer", "minimum": 0},
                    "max_depth": {"type": ["integer", "null"], "minimum": 0},
                    "has_more": {"type": "boolean"},
                    "continuation": {"type": ["string", "null"]}, "truncated": {"type": "boolean"}
                }}
        }
    })
    .as_object()
    .expect("get output schema is an object")
    .clone()
}

fn check_output_schema() -> Map<String, Value> {
    serde_json::json!({
        "type": "object", "additionalProperties": false, "required": ["categories"],
        "properties": {"categories": {"type": "array", "items": {
            "type": "object", "additionalProperties": false,
            "required": ["category", "status", "findings"],
            "properties": {
                "category": {"type": "string", "enum": ["graph", "statements", "bindings"]},
                "status": {"type": "string", "enum": ["passed", "findings", "unavailable"]},
                "findings": {"type": "array", "items": {"type": "object",
                    "additionalProperties": false, "required": ["message"],
                    "properties": {"message": {"type": "string"}, "detail": {}}}},
                "unavailable_reason": {"type": "string"}, "context": {}
            }
        }}}
    })
    .as_object()
    .expect("check output schema is an object")
    .clone()
}

pub(crate) async fn call_check(
    host: &crate::StatementHost,
    port: std::sync::Arc<dyn provenance_porcelain::check::CheckPort>,
    arguments: serde_json::Map<String, Value>,
) -> CallToolResult {
    let Ok(arguments) = serde_json::from_value::<CheckArguments>(Value::Object(arguments)) else {
        return get_error("invalid_options", "unsupported check options");
    };
    let service = provenance_porcelain::Porcelain::new(port);
    let mut input = arguments.into_check_input();
    if let Some((_, scope)) = host.bound_identity() {
        input = input.in_scope(scope);
    }
    let outcome = service.check(input).await;
    let summary = outcome
        .categories
        .iter()
        .flat_map(|report| {
            let heading =
                format!("{:?}: {:?}", report.category, report.status).to_ascii_lowercase();
            std::iter::once(heading)
                .chain(
                    report
                        .findings
                        .iter()
                        .map(|finding| format!("  - {}", finding.message)),
                )
                .chain(
                    report
                        .unavailable_reason
                        .iter()
                        .map(|reason| format!("  - {reason}")),
                )
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut result =
        CallToolResult::structured(serde_json::to_value(outcome).expect("check outcome is JSON"));
    result.content = vec![Content::text(summary)];
    result
}

/// Returns readable and structured MCP content for one get request.
pub(crate) async fn call_get(
    host: &crate::StatementHost,
    arguments: serde_json::Map<String, Value>,
) -> CallToolResult {
    let Ok(arguments) = serde_json::from_value::<GetArguments>(Value::Object(arguments)) else {
        return get_error("invalid_options", "unsupported read options");
    };
    let service = provenance_porcelain::Porcelain::new(HostGetPort::new(host.clone()));
    match service.get(arguments.into_get_input()).await {
        Ok(outcome) => {
            let summary = render_get_readable(&outcome);
            let mut result = CallToolResult::structured(
                serde_json::to_value(outcome).expect("get outcome is JSON"),
            );
            result.content = vec![Content::text(summary)];
            result
        }
        Err(error) => get_error(
            match error {
                ReadError::InvalidOptions => "invalid_options",
                ReadError::NotFound => "not_found",
                ReadError::AmbiguousIdentity => "ambiguous_identity",
                ReadError::Operation(_) => "operation_failed",
            },
            &error.to_string(),
        ),
    }
}

pub fn render_get_readable(outcome: &provenance_porcelain::get::GetOutcome) -> String {
    let mut lines = vec![
        format!(
            "{} {}",
            outcome.record.node_type().as_str(),
            outcome.record.id().as_str()
        ),
        format!("view: {:?}", outcome.view()).to_ascii_lowercase(),
        format!(
            "record: {}",
            serde_json::to_string_pretty(&provenance_porcelain::get::RecordData(&outcome.record))
                .expect("record values are valid JSON")
        ),
    ];
    if !outcome.related().is_empty() {
        lines.push("related:".to_owned());
        lines.extend(outcome.related().iter().map(|record| {
            format!(
                "- {} {}: {}",
                record.node.node_type().as_str(),
                record.node.id().as_str(),
                serde_json::to_string(&record.node).expect("record values are valid JSON")
            )
        }));
    }
    if let Some(detail) = outcome.impact() {
        lines.push(format!(
            "detail: {}",
            serde_json::to_string_pretty(detail).expect("view details are valid JSON")
        ));
    }
    if let Some(bounds) = outcome.bounds() {
        lines.push(format!(
            "bounds: limit={} max_depth={} has_more={} truncated={} continuation={}",
            bounds.limit,
            bounds
                .max_depth
                .map_or_else(|| "none".to_owned(), |depth| depth.to_string()),
            bounds.has_more,
            bounds.truncated,
            bounds.continuation.as_deref().unwrap_or("none")
        ));
    }
    for (label, metadata) in [
        ("record", outcome.record_metadata.as_ref()),
        ("view", outcome.view_metadata()),
    ] {
        if let Some(error) = metadata.and_then(|value| value.freshness_error.as_deref()) {
            lines.push(format!("warning: {label} freshness: {error}"));
        }
    }
    lines.join("\n")
}

fn get_error(kind: &str, message: &str) -> CallToolResult {
    CallToolResult::structured_error(serde_json::json!({
        "error": {"kind": kind, "message": message}
    }))
}

/// MCP input for the `get` Porcelain action.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GetArguments {
    /// The repository-local record ID.
    pub target: String,
    /// The named read view.
    #[serde(default)]
    pub view: View,
    /// The maximum child or grounding traversal depth.
    #[serde(default)]
    pub max_depth: Option<usize>,
    /// The record kinds returned after traversal.
    #[serde(default)]
    pub returned_kinds: Vec<provenance_core::NodeType>,
    /// The maximum number of view results.
    #[serde(default)]
    pub limit: Option<usize>,
}

impl GetArguments {
    /// Translate the MCP input into a shared semantic get request.
    pub fn into_get_input(self) -> GetInput {
        GetInput {
            target: self.target,
            view: self.view,
            max_depth: self.max_depth,
            returned_kinds: self.returned_kinds,
            limit: self.limit,
        }
    }
}
