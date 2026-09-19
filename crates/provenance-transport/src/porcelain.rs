//! MCP-owned bindings for shared Porcelain capabilities.

use axum::http::{HeaderMap, Method};
use provenance_porcelain::get::{
    Bounds, GetInput, GetPort, Impact, PortFuture, ReadError, Record, Traversal, TraversalRequest,
    View,
};
use provenance_porcelain::{Action, Outcome, RecordRequest};
use rmcp::model::{CallToolResult, Content};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

const ACTION_NAMES: &[&str] = &["get", "check"];
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
            for (kind, collection, _) in RECORD_KINDS {
                let path = format!("/{collection}/{id}");
                match self.query(&path, BTreeMap::new()).await {
                    Ok(value) => {
                        let data = value.get("data").cloned().ok_or_else(malformed)?;
                        if found.is_some() {
                            return Err(ReadError::AmbiguousIdentity);
                        }
                        found = Some(Record::new(id, kind, data));
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
                .ok_or_else(malformed)?
                .iter()
                .map(|entry| record_from_node(&entry["node"]))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Traversal {
                records,
                bounds: bounds(&value, Some(request.max_depth)),
            })
        })
    }

    fn impact<'a>(&'a self, id: &'a str, limit: usize) -> PortFuture<'a, Impact> {
        Box::pin(async move {
            let record = self.resolve(id).await?.ok_or(ReadError::NotFound)?;
            let collection = collection(&record.kind)?;
            let query = BTreeMap::from([
                ("query".to_owned(), "impact".to_owned()),
                ("limit".to_owned(), limit.to_string()),
            ]);
            let value = self.query(&format!("/{collection}/{id}"), query).await?;
            let detail = value.get("data").cloned().ok_or_else(malformed)?;
            let mut result_bounds = bounds(&value, None);
            result_bounds.truncated |= detail
                .get("scan_cut")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Ok(Impact {
                detail,
                bounds: result_bounds,
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

/// Return the Porcelain action names exposed by the MCP binding.
pub const fn action_names() -> &'static [&'static str] {
    ACTION_NAMES
}

pub(crate) fn get_is_available(host: &crate::StatementHost) -> bool {
    RECORD_KINDS
        .iter()
        .all(|(_, _, operation)| host.advertises(operation))
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
    rmcp::model::Tool::new(
        "get",
        "Read one repository record by its repository-local ID.",
        schema.as_object().expect("get schema is an object").clone(),
    )
}

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
            let summary = format!("{} {}", outcome.record.kind, outcome.record.id);
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
    /// Translate the MCP input into a shared semantic request.
    pub fn into_request(self) -> RecordRequest {
        RecordRequest::new(self.target, Action::Get)
    }

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

/// Render a shared outcome in the MCP-owned result shape.
pub fn render<T: Serialize>(outcome: Outcome<T>) -> serde_json::Result<CallToolResult> {
    let mut result = CallToolResult::structured(serde_json::to_value(outcome.data)?);
    result.content = vec![Content::text(outcome.summary)];
    Ok(result)
}
