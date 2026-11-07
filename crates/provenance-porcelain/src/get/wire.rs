//! The get wire shape and its schema come from the same typed projection.

use super::{Bounds, GetInput, GetOutcome, Registration, Review, View};
use provenance_core::protocol::{GraphNode, ImpactResult, ResponseMeta};
use provenance_core::{
    Boundary, Domain, NodeType, Question, Requirement, Resolution, Rule, Source, StableId, Topic,
};
use schemars::generate::{Contract, SchemaSettings};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

/// A canonical record payload without the graph node's variant tag.
#[derive(schemars::JsonSchema, Serialize)]
#[schemars(description = "The record fields, without the node_type field.")]
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
    review: Option<&'a Review>,
    bounds: Option<&'a Bounds>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shaping: Option<&'a super::SessionSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    record_metadata: Option<&'a ResponseMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    view_metadata: Option<&'a ResponseMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notify: Option<&'a Registration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    review_link: Option<&'a super::ReviewLink>,
}

/// The view of a trace result.
#[derive(schemars::JsonSchema, Serialize)]
#[serde(rename_all = "snake_case")]
enum TraceView {
    Trace,
}

/// The shape of a get result for the trace view: the fields of [`GetWire`] that a trace
/// fills. A trace has no impact detail and no review, so those two fields are null, and
/// a trace has no shaping summary, notify registration, or review link. A trace result
/// serializes through this shape, so its schema is the shape of each trace result.
#[derive(schemars::JsonSchema, Serialize)]
#[schemars(description = "A get result for the trace view.")]
#[serde(deny_unknown_fields)]
struct TraceWire<'a> {
    record: RecordWire<'a>,
    view: TraceView,
    related: Vec<RelatedWire<'a>>,
    detail: (),
    review: (),
    bounds: &'a Bounds,
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
            related: self
                .related()
                .iter()
                .map(|record| RelatedWire {
                    id: record.node.id(),
                    kind: record.node.node_type(),
                    value: &record.node,
                    depth: record.depth,
                })
                .collect(),
            detail: self.impact(),
            review: self.review(),
            bounds: self.bounds(),
            shaping: self.shaping(),
            record_metadata: self.record_metadata.as_ref(),
            view_metadata: self.view_metadata(),
            notify: self.review().and_then(|review| review.notify.as_ref()),
            review_link: match &self.result {
                super::ViewResult::ReviewLink(link) => Some(link),
                _ => None,
            },
        }
    }
}

impl GetOutcome {
    fn trace_wire<'a>(&'a self, bounds: &'a Bounds) -> TraceWire<'a> {
        let wire = self.wire();
        TraceWire {
            record: wire.record,
            view: TraceView::Trace,
            related: wire.related,
            detail: (),
            review: (),
            bounds,
            record_metadata: wire.record_metadata,
            view_metadata: wire.view_metadata,
        }
    }
}

impl Serialize for GetOutcome {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match &self.result {
            super::ViewResult::Trace(traversal) => {
                self.trace_wire(&traversal.bounds).serialize(serializer)
            }
            _ => self.wire().serialize(serializer),
        }
    }
}

fn schema<T: schemars::JsonSchema>(contract: Contract) -> Value {
    serde_json::to_value(
        SchemaSettings::draft2020_12()
            .with(|settings| settings.contract = contract)
            .into_generator()
            .into_root_schema_for::<T>(),
    )
    .expect("typed get schema is JSON")
}

/// The get request schema: the target and the get fields.
pub fn input_schema() -> Value {
    schema::<GetInput>(Contract::Deserialize)
}

/// The trace request schema: the target and the trace fields.
pub fn trace_input_schema() -> Value {
    schema::<TraceInput>(Contract::Deserialize)
}

/// One trace request: the records above the target, up to its Sources.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TraceInput {
    #[schemars(length(min = 1))]
    pub target: String,
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub depth: Option<usize>,
    #[serde(default)]
    pub kind: Vec<NodeType>,
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub limit: Option<usize>,
}

impl From<TraceInput> for GetInput {
    fn from(input: TraceInput) -> Self {
        let mut get = Self::new(input.target, View::Trace);
        get.depth = input.depth;
        get.kind = input.kind;
        get.limit = input.limit;
        get
    }
}

/// The notify request schema: the target and the get fields that write.
pub fn notify_input_schema() -> Value {
    schema::<NotifyInput>(Contract::Deserialize)
}

/// One notify request: the client thread to register for the review changes of one
/// record, or `off` to end that registration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NotifyInput {
    #[schemars(length(min = 1))]
    pub target: String,
    pub notify: super::NotifyClient,
    #[schemars(length(min = 1))]
    pub thread: String,
}

impl From<NotifyInput> for GetInput {
    fn from(input: NotifyInput) -> Self {
        let mut get = Self::new(input.target, View::Review);
        get.notify = Some(input.notify);
        get.thread = Some(input.thread);
        get
    }
}

/// The notify result schema.
pub fn notify_output_schema() -> Value {
    schema::<NotifyResult>(Contract::Serialize)
}

/// The result of one notify request.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NotifyResult {
    pub notify: super::Registration,
}

/// The get result schema is generated from the serialized projection.
pub fn output_schema() -> Value {
    schema::<GetWire<'static>>(Contract::Serialize)
}

/// The schema of a get result for the trace view.
pub fn trace_output_schema() -> Value {
    schema::<TraceWire<'static>>(Contract::Serialize)
}

/// Render a get result as readable text.
pub fn render_readable(outcome: &GetOutcome) -> serde_json::Result<String> {
    let mut sections = vec![
        format!(
            "{} {}",
            outcome.record.node_type().as_str(),
            outcome.record.id().as_str()
        ),
        format!("view: {}", outcome.view().as_str()),
        format!(
            "record:\n{}",
            serde_json::to_string_pretty(&RecordData::from(&outcome.record))?
        ),
    ];
    if !outcome.related().is_empty() {
        sections.push(format!(
            "related:\n{}",
            outcome
                .related()
                .iter()
                .map(|record| format!(
                    "- {} {}: {}",
                    record.node.node_type().as_str(),
                    record.node.id().as_str(),
                    serde_json::to_string(&record.node).expect("record values are JSON")
                ))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    if let Some(summary) = outcome.shaping() {
        sections.push(format!(
            "shaping:\n{}",
            serde_json::to_string_pretty(summary)?
        ));
    }
    if let Some(detail) = outcome.impact() {
        sections.push(format!(
            "detail:\n{}",
            serde_json::to_string_pretty(detail)?
        ));
    }
    if let Some(review) = outcome.review() {
        sections.push(format!(
            "update precondition: {}\nreview:\n{}",
            review.update_precondition,
            serde_json::to_string_pretty(review)?
        ));
        if let Some(notify) = &review.notify {
            sections.push(format!(
                "notify:\n{}",
                serde_json::to_string_pretty(notify)?
            ));
        }
    }
    if let super::ViewResult::ReviewLink(link) = &outcome.result {
        sections.push(match (&link.review_url, &link.message) {
            (Some(url), _) => format!("review link: {url}"),
            (None, Some(message)) => format!("review link: none. {message}"),
            (None, None) => "review link: none".to_owned(),
        });
    }
    if let Some(bounds) = outcome.bounds() {
        sections.push(format!(
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
            sections.push(format!("warning: {label} freshness: {error}"));
        }
    }
    Ok(sections.join("\n\n"))
}
