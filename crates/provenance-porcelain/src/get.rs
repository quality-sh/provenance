//! Composed reads for one known repository record.

use provenance_core::protocol::{
    GraphNode, ImpactResult, RecordResolution as CoreRecordResolution, ResponseMeta, TracedNode,
};
use provenance_core::NodeType;
use provenance_macros::rule;
use serde::{Deserialize, Serialize, Serializer};
use std::{fmt::Display, future::Future, pin::Pin};

/// One future returned by an injected read port.
pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ReadError>> + Send + 'a>>;

/// The result of resolving one repository-local record ID.
#[derive(Clone, Debug)]
pub struct RecordResolution {
    pub result: CoreRecordResolution,
    pub metadata: Option<ResponseMeta>,
}

/// The named view selected for a known record.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum View {
    #[default]
    Record,
    Children,
    Grounding,
    Impact,
}

/// One semantic known-record request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GetInput {
    pub target: String,
    pub view: View,
    pub max_depth: Option<usize>,
    pub returned_kinds: Vec<NodeType>,
    pub limit: Option<usize>,
}

impl GetInput {
    pub fn new(target: impl Into<String>, view: View) -> Self {
        Self {
            target: target.into(),
            view,
            max_depth: None,
            returned_kinds: Vec::new(),
            limit: None,
        }
    }
}

/// The bound and continuation state of a potentially incomplete result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Bounds {
    pub limit: usize,
    pub max_depth: Option<usize>,
    pub has_more: bool,
    pub continuation: Option<String>,
    pub truncated: bool,
}

/// A traversal request sent through the injected operation port.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraversalRequest {
    pub target: String,
    pub kind: NodeType,
    pub view: View,
    pub max_depth: usize,
    pub limit: usize,
}

/// A typed traversal result before returned-kind filtering.
#[derive(Clone, Debug)]
pub struct Traversal {
    pub records: Vec<TracedNode>,
    pub bounds: Bounds,
    pub response_metadata: Option<ResponseMeta>,
}

/// A typed impact result with its page state.
#[derive(Clone, Debug)]
pub struct Impact {
    pub detail: ImpactResult,
    pub bounds: Bounds,
    pub response_metadata: Option<ResponseMeta>,
}

/// The data selected by one named view.
#[derive(Clone, Debug)]
pub enum ViewResult {
    Record,
    Children(Traversal),
    Grounding(Traversal),
    Impact(Impact),
}

impl ViewResult {
    pub const fn view(&self) -> View {
        match self {
            Self::Record => View::Record,
            Self::Children(_) => View::Children,
            Self::Grounding(_) => View::Grounding,
            Self::Impact(_) => View::Impact,
        }
    }

    pub fn related(&self) -> &[TracedNode] {
        match self {
            Self::Children(traversal) | Self::Grounding(traversal) => &traversal.records,
            Self::Record | Self::Impact(_) => &[],
        }
    }

    pub const fn impact(&self) -> Option<&ImpactResult> {
        match self {
            Self::Impact(impact) => Some(&impact.detail),
            _ => None,
        }
    }

    pub const fn bounds(&self) -> Option<&Bounds> {
        match self {
            Self::Children(traversal) | Self::Grounding(traversal) => Some(&traversal.bounds),
            Self::Impact(impact) => Some(&impact.bounds),
            Self::Record => None,
        }
    }

    pub const fn metadata(&self) -> Option<&ResponseMeta> {
        match self {
            Self::Children(traversal) | Self::Grounding(traversal) => {
                traversal.response_metadata.as_ref()
            }
            Self::Impact(impact) => impact.response_metadata.as_ref(),
            Self::Record => None,
        }
    }
}

/// A composed known-record result with one typed view payload.
#[derive(Clone, Debug)]
pub struct GetOutcome {
    pub record: GraphNode,
    pub result: ViewResult,
    pub record_metadata: Option<ResponseMeta>,
}

impl GetOutcome {
    pub const fn view(&self) -> View {
        self.result.view()
    }

    pub fn related(&self) -> &[TracedNode] {
        self.result.related()
    }

    pub const fn impact(&self) -> Option<&ImpactResult> {
        self.result.impact()
    }

    pub const fn bounds(&self) -> Option<&Bounds> {
        self.result.bounds()
    }

    pub const fn view_metadata(&self) -> Option<&ResponseMeta> {
        self.result.metadata()
    }
}

/// Serializes the canonical payload without the `GraphNode` discriminant.
pub struct RecordData<'a>(pub &'a GraphNode);

impl Serialize for RecordData<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            GraphNode::Source(record) => record.serialize(serializer),
            GraphNode::Requirement(record) => record.serialize(serializer),
            GraphNode::Resolution(record) => record.serialize(serializer),
            GraphNode::Rule(record) => record.serialize(serializer),
            GraphNode::Topic(record) => record.serialize(serializer),
            GraphNode::Question(record) => record.serialize(serializer),
            GraphNode::Domain(record) => record.serialize(serializer),
            GraphNode::Boundary(record) => record.serialize(serializer),
        }
    }
}

#[derive(Serialize)]
struct OutputRecord<'a> {
    id: &'a str,
    kind: &'static str,
    value: RecordData<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    depth: Option<usize>,
}

fn output_record(node: &GraphNode, depth: Option<usize>) -> OutputRecord<'_> {
    OutputRecord {
        id: node.id().as_str(),
        kind: node.node_type().as_str(),
        value: RecordData(node),
        depth,
    }
}

impl Serialize for GetOutcome {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct as _;
        let fields = 5
            + usize::from(self.record_metadata.is_some())
            + usize::from(self.view_metadata().is_some());
        let mut output = serializer.serialize_struct("GetOutcome", fields)?;
        output.serialize_field("record", &output_record(&self.record, None))?;
        output.serialize_field("view", &self.view())?;
        let related = self
            .related()
            .iter()
            .map(|record| output_record(&record.node, Some(record.depth)))
            .collect::<Vec<_>>();
        output.serialize_field("related", &related)?;
        output.serialize_field("detail", &self.impact())?;
        output.serialize_field("bounds", &self.bounds())?;
        if let Some(metadata) = &self.record_metadata {
            output.serialize_field("record_metadata", metadata)?;
        }
        if let Some(metadata) = self.view_metadata() {
            output.serialize_field("view_metadata", metadata)?;
        }
        output.end()
    }
}

/// A failure from validation, identity resolution, or an injected operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadError {
    InvalidOptions,
    NotFound,
    AmbiguousIdentity,
    Operation(String),
}

impl Display for ReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOptions => formatter.write_str("unsupported read options"),
            Self::NotFound => formatter.write_str("record does not exist"),
            Self::AmbiguousIdentity => formatter.write_str("record ID is not unique"),
            Self::Operation(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ReadError {}

/// Existing operation paths needed to compose a known-record read.
pub trait GetPort: Send + Sync {
    fn resolve<'a>(&'a self, id: &'a str) -> PortFuture<'a, RecordResolution>;
    fn traverse(&self, request: TraversalRequest) -> PortFuture<'_, Traversal>;
    fn impact<'a>(&'a self, record: &'a GraphNode, limit: usize) -> PortFuture<'a, Impact>;
}

impl<P: GetPort> crate::Porcelain<P> {
    /// Resolves one record and composes its selected human-facing view.
    #[rule("rule_porcelain_id_needs_no_kind_selector")]
    #[rule("rule_porcelain_get_returns_record")]
    #[rule("rule_porcelain_get_selects_child_context")]
    #[rule("rule_porcelain_return_filter_keeps_traversal")]
    #[rule("rule_porcelain_get_has_grounding_impact")]
    #[rule("rule_porcelain_read_rejects_bad_options")]
    pub async fn get(&self, input: GetInput) -> Result<GetOutcome, ReadError> {
        validate(&input)?;
        let resolved = self.port.resolve(&input.target).await?;
        let record = match resolved.result {
            CoreRecordResolution::Found(node) => node,
            CoreRecordResolution::Missing => return Err(ReadError::NotFound),
            CoreRecordResolution::Ambiguous => return Err(ReadError::AmbiguousIdentity),
        };
        let record_metadata = resolved.metadata;
        let result = match input.view {
            View::Record => ViewResult::Record,
            View::Children | View::Grounding => {
                let mut traversal = self
                    .port
                    .traverse(TraversalRequest {
                        target: input.target,
                        kind: record.node_type(),
                        view: input.view,
                        max_depth: input.max_depth.unwrap_or(1),
                        limit: input.limit.unwrap_or(50),
                    })
                    .await?;
                traversal.records.retain(|candidate| {
                    input.returned_kinds.is_empty()
                        || input.returned_kinds.contains(&candidate.node.node_type())
                });
                if input.view == View::Children {
                    ViewResult::Children(traversal)
                } else {
                    ViewResult::Grounding(traversal)
                }
            }
            View::Impact => {
                ViewResult::Impact(self.port.impact(&record, input.limit.unwrap_or(50)).await?)
            }
        };
        Ok(GetOutcome {
            record,
            result,
            record_metadata,
        })
    }
}

fn validate(input: &GetInput) -> Result<(), ReadError> {
    let has_traversal_options = input.max_depth.is_some() || !input.returned_kinds.is_empty();
    let unsupported = match input.view {
        View::Record => has_traversal_options || input.limit.is_some(),
        View::Children | View::Grounding => false,
        View::Impact => has_traversal_options,
    };
    if unsupported || input.max_depth == Some(0) || input.limit == Some(0) {
        return Err(ReadError::InvalidOptions);
    }
    Ok(())
}
