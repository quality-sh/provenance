//! The get wire shape and its schema come from the same typed projection.

use super::{Bounds, GetInput, GetOutcome, View};
use provenance_core::protocol::{GraphNode, ImpactResult, ResponseMeta};
use provenance_core::{Boundary, Domain, NodeType, Question, Requirement, Resolution, Rule, Source, StableId, Topic};
use schemars::generate::{Contract, SchemaSettings};
use serde::{Serialize, Serializer};
use serde_json::Value;

/// A canonical record payload without the graph node's variant tag.
#[derive(schemars::JsonSchema, Serialize)]
#[serde(untagged)]
enum RecordData<'a> {
    Source(&'a Source),
    Requirement(&'a Requirement),
    Resolution(&'a Resolution),
    Rule(&'a Rule),
    Topic(&'a Topic),
    Question(&'a Question),
    Domain(&'a Domain),
    Boundary(&'a Boundary),
}

impl<'a> From<&'a GraphNode> for RecordData<'a> {
    fn from(node: &'a GraphNode) -> Self {
        match node {
            GraphNode::Source(record) => Self::Source(record),
            GraphNode::Requirement(record) => Self::Requirement(record),
            GraphNode::Resolution(record) => Self::Resolution(record),
            GraphNode::Rule(record) => Self::Rule(record),
            GraphNode::Topic(record) => Self::Topic(record),
            GraphNode::Question(record) => Self::Question(record),
            GraphNode::Domain(record) => Self::Domain(record),
            GraphNode::Boundary(record) => Self::Boundary(record),
        }
    }
}

#[derive(schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct RecordWire<'a> {
    id: &'a StableId,
    kind: NodeType,
    value: RecordData<'a>,
}

#[derive(schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct RelatedWire<'a> {
    id: &'a StableId,
    kind: NodeType,
    value: &'a GraphNode,
    #[schemars(range(min = 1))]
    depth: usize,
}

#[derive(schemars::JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
struct GetWire<'a> {
    record: RecordWire<'a>,
    view: View,
    related: Vec<RelatedWire<'a>>,
    detail: Option<&'a ImpactResult>,
    bounds: Option<&'a Bounds>,
    #[serde(skip_serializing_if = "Option::is_none")]
    record_metadata: Option<&'a ResponseMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    view_metadata: Option<&'a ResponseMeta>,
}

impl GetOutcome {
    fn wire(&self) -> GetWire<'_> {
        GetWire {
            record: RecordWire {
                id: self.record.id(),
                kind: self.record.node_type(),
                value: RecordData::from(&self.record),
            },
            view: self.view(),
            related: self.related().iter().map(|record| RelatedWire {
                id: record.node.id(),
                kind: record.node.node_type(),
                value: &record.node,
                depth: record.depth,
            }).collect(),
            detail: self.impact(),
            bounds: self.bounds(),
            record_metadata: self.record_metadata.as_ref(),
            view_metadata: self.view_metadata(),
        }
    }
}

impl Serialize for GetOutcome {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.wire().serialize(serializer)
    }
}

fn schema<T: schemars::JsonSchema>(contract: Contract) -> Value {
    serde_json::to_value(
        SchemaSettings::draft2020_12()
            .with(|settings| settings.contract = contract)
            .into_generator()
            .into_root_schema_for::<T>(),
    ).expect("typed get schema is JSON")
}

/// The MCP get request schema is generated from the deserialized request.
pub fn input_schema() -> Value {
    schema::<GetInput>(Contract::Deserialize)
}

/// The MCP get result schema is generated from the serialized projection.
pub fn output_schema() -> Value {
    schema::<GetWire<'static>>(Contract::Serialize)
}

/// Render a get result for both terminal and MCP readers.
pub fn render_readable(outcome: &GetOutcome) -> serde_json::Result<String> {
    let mut sections = vec![
        format!("{} {}", outcome.record.node_type().as_str(), outcome.record.id().as_str()),
        format!("view: {}", outcome.view().as_str()),
        format!("record:\n{}", serde_json::to_string_pretty(&RecordData::from(&outcome.record))?),
    ];
    if !outcome.related().is_empty() {
        sections.push(format!("related:\n{}", outcome.related().iter().map(|record| format!(
            "- {} {}: {}", record.node.node_type().as_str(), record.node.id().as_str(),
            serde_json::to_string(&record.node).expect("record values are JSON")
        )).collect::<Vec<_>>().join("\n")));
    }
    if let Some(detail) = outcome.impact() {
        sections.push(format!("detail:\n{}", serde_json::to_string_pretty(detail)?));
    }
    if let Some(bounds) = outcome.bounds() {
        sections.push(format!(
            "bounds: limit={} max_depth={} has_more={} truncated={} continuation={}",
            bounds.limit,
            bounds.max_depth.map_or_else(|| "none".to_owned(), |depth| depth.to_string()),
            bounds.has_more, bounds.truncated, bounds.continuation.as_deref().unwrap_or("none")
        ));
    }
    for (label, metadata) in [("record", outcome.record_metadata.as_ref()), ("view", outcome.view_metadata())] {
        if let Some(error) = metadata.and_then(|value| value.freshness_error.as_deref()) {
            sections.push(format!("warning: {label} freshness: {error}"));
        }
    }
    Ok(sections.join("\n\n"))
}
