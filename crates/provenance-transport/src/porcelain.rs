//! MCP-owned bindings for shared Porcelain capabilities.

use axum::http::{HeaderMap, Method};
use provenance_macros::rule;
use provenance_porcelain::check::{Category, CheckInput};
use provenance_porcelain::get::{
    Bounds, GetInput, GetPort, Impact, PortFuture, ReadError, Record, Traversal, TraversalRequest,
    View,
};
use rmcp::model::{CallToolResult, Content};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

const RECORD_KINDS: [(&str, &str, &str); 8] = [
    ("source", "sources", "get-source"),
    ("requirement", "requirements", "get-requirement"),
    ("resolution", "resolutions", "get-resolution"),
    ("rule", "rules", "get-rule"),
    ("topic", "topics", "get-topic"),
    ("question", "questions", "get-question"),
    ("domain", "domains", "get-domain"),
    ("boundary", "boundaries", "get-boundary"),
];

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

/// Existing resource routes adapted to the injected Porcelain read port.
#[derive(Clone)]
pub struct HostGetPort {
    host: crate::StatementHost,
}

impl HostGetPort {
    pub const fn new(host: crate::StatementHost) -> Self {
        Self { host }
    }

    async fn query(&self, path: &str, query: BTreeMap<String, String>) -> Result<Value, ReadError> {
        self.host
            .invoke_resource(
                Method::GET,
                path,
                Value::Object(Map::default()),
                query,
                HeaderMap::new(),
            )
            .await
            .map_err(|error| operation_error(&error))
    }
}

impl GetPort for HostGetPort {
    fn resolve<'a>(&'a self, id: &'a str) -> PortFuture<'a, Option<Record>> {
        Box::pin(async move {
            let mut found = None;
            for (kind, collection, operation) in RECORD_KINDS {
                if !self.host.advertises(operation) {
                    continue;
                }
                let path = format!("/{collection}/{id}");
                match self.query(&path, BTreeMap::new()).await {
                    Ok(value) => {
                        let data = value.get("data").cloned().ok_or_else(malformed)?;
                        if found.is_some() {
                            return Err(ReadError::AmbiguousIdentity);
                        }
                        let metadata = value.get("meta").cloned().ok_or_else(malformed)?;
                        found = Some(Record::new(id, kind, data).with_response_metadata(metadata));
                    }
                    Err(ReadError::NotFound) => {}
                    Err(error) => return Err(error),
                }
            }
            Ok(found)
        })
    }

    fn traverse(&self, request: TraversalRequest) -> PortFuture<'_, Traversal> {
        Box::pin(async move {
            let collection = collection(&request.kind)?;
            let mut query = BTreeMap::from([
                ("query".to_owned(), "trace".to_owned()),
                ("limit".to_owned(), request.limit.to_string()),
                ("max_depth".to_owned(), request.max_depth.to_string()),
            ]);
            query.insert(
                "direction".to_owned(),
                if request.view == View::Children {
                    "in"
                } else {
                    "out"
                }
                .to_owned(),
            );
            let value = self
                .query(&format!("/{collection}/{}", request.target), query)
                .await?;
            let records = value
                .pointer("/data/nodes")
                .and_then(Value::as_array)
                .ok_or_else(malformed)?;
            let mut permitted = Vec::new();
            for entry in records {
                let record = record_from_node(&entry["node"])?;
                if operation_for_kind(&record.kind)
                    .is_some_and(|operation| self.host.advertises(operation))
                {
                    permitted.push(record);
                }
            }
            Ok(Traversal {
                records: permitted,
                bounds: bounds(&value, Some(request.max_depth)),
                response_metadata: value.get("meta").cloned(),
            })
        })
    }

    fn impact<'a>(&'a self, record: &'a Record, limit: usize) -> PortFuture<'a, Impact> {
        Box::pin(async move {
            if !RECORD_KINDS
                .iter()
                .all(|(_, _, operation)| self.host.advertises(operation))
            {
                return Err(ReadError::InvalidOptions);
            }
            let collection = collection(&record.kind)?;
            let query = BTreeMap::from([
                ("query".to_owned(), "impact".to_owned()),
                ("limit".to_owned(), limit.to_string()),
            ]);
            let value = self
                .query(&format!("/{collection}/{}", record.id), query)
                .await?;
            let detail = value.get("data").cloned().ok_or_else(malformed)?;
            let mut result_bounds = bounds(&value, None);
            result_bounds.truncated |= detail
                .get("scan_cut")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Ok(Impact {
                detail,
                bounds: result_bounds,
                response_metadata: value.get("meta").cloned(),
            })
        })
    }
}

fn collection(kind: &str) -> Result<&'static str, ReadError> {
    RECORD_KINDS
        .iter()
        .find_map(|(candidate, collection, _)| (*candidate == kind).then_some(*collection))
        .ok_or(ReadError::InvalidOptions)
}

fn operation_for_kind(kind: &str) -> Option<&'static str> {
    RECORD_KINDS
        .iter()
        .find_map(|(candidate, _, operation)| (*candidate == kind).then_some(*operation))
}

fn record_from_node(value: &Value) -> Result<Record, ReadError> {
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(malformed)?;
    let kind = value
        .get("node_type")
        .and_then(Value::as_str)
        .ok_or_else(malformed)?;
    Ok(Record::new(id, kind, value.clone()))
}

fn bounds(value: &Value, max_depth: Option<usize>) -> Bounds {
    let limit = usize::try_from(
        value
            .pointer("/meta/limit")
            .and_then(Value::as_u64)
            .unwrap_or(0),
    )
    .unwrap_or(usize::MAX);
    let has_more = value
        .pointer("/meta/has_more")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Bounds {
        limit,
        max_depth,
        has_more,
        continuation: value
            .pointer("/meta/next_cursor")
            .and_then(Value::as_str)
            .map(str::to_owned),
        truncated: has_more,
    }
}

fn malformed() -> ReadError {
    ReadError::Operation("operation returned an invalid get result".to_owned())
}

fn operation_error(error: &provenance_core::protocol::failure::ErasedFailure) -> ReadError {
    if error.error.get("kind").and_then(Value::as_str) == Some("resource_not_found") {
        ReadError::NotFound
    } else {
        ReadError::Operation(serde_json::to_string(error).unwrap_or_default())
    }
}

pub(crate) fn get_is_available(host: &crate::StatementHost) -> bool {
    RECORD_KINDS
        .iter()
        .any(|(_, _, operation)| host.advertises(operation))
}

pub(crate) fn get_tool() -> rmcp::model::Tool {
    let schema = serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["target"],
        "properties": {
            "target": {"type": "string", "minLength": 1},
            "view": {"type": "string", "enum": ["record", "children", "grounding", "impact"], "default": "record"},
            "max_depth": {"type": "integer", "minimum": 1},
            "returned_kinds": {"type": "array", "items": {"type": "string", "minLength": 1}},
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
                    "id": {"type": "string"}, "kind": {"type": "string"}, "value": {}
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
        .map(|report| format!("{:?}: {:?}", report.category, report.status).to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("; ");
    let mut result =
        CallToolResult::structured(serde_json::to_value(outcome).expect("check outcome is JSON"));
    result.content = vec![Content::text(summary)];
    result
}

/// Returns readable and structured MCP content for one get request.
#[rule("rule_porcelain_mcp_readable_structured")]
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

fn render_get_readable(outcome: &provenance_porcelain::get::GetOutcome) -> String {
    let mut lines = vec![
        format!("{} {}", outcome.record.kind, outcome.record.id),
        format!("view: {:?}", outcome.view).to_ascii_lowercase(),
        format!(
            "record: {}",
            serde_json::to_string_pretty(&outcome.record.value)
                .expect("record values are valid JSON")
        ),
    ];
    if !outcome.related.is_empty() {
        lines.push("related:".to_owned());
        lines.extend(outcome.related.iter().map(|record| {
            format!(
                "- {} {}: {}",
                record.kind,
                record.id,
                serde_json::to_string(&record.value).expect("record values are valid JSON")
            )
        }));
    }
    if let Some(detail) = &outcome.detail {
        lines.push(format!(
            "detail: {}",
            serde_json::to_string_pretty(detail).expect("view details are valid JSON")
        ));
    }
    if let Some(bounds) = &outcome.bounds {
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
    pub returned_kinds: Vec<String>,
    /// The maximum number of view results.
    #[serde(default)]
    pub limit: Option<usize>,
}

impl GetArguments {
    /// Translate the MCP input into a shared semantic get request.
    #[rule("rule_porcelain_mcp_target_argument")]
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
