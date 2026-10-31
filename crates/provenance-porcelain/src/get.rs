//! Composed reads for one known repository record.

use provenance_core::protocol::{
    GraphNode, ImpactResult, RecordResolution as CoreRecordResolution, ResponseMeta, TracedNode,
};
use provenance_core::{
    review::{RequirementDecisionState, RequirementEditState},
    threads::{DiscussionConversationResult, DiscussionResultPage},
    NodeType,
};
use provenance_macros::rule;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, future::Future, pin::Pin};

mod shaping;
mod wire;
pub use shaping::{BlockedQuestion, BlockedReason, Claim, QuestionState, SessionSummary};
pub use wire::{input_schema, output_schema, render_readable};

/// One future returned by an injected read port.
pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ReadError>> + Send + 'a>>;

/// The result of resolving one repository-local record ID.
#[derive(Clone, Debug)]
pub struct RecordResolution {
    pub result: CoreRecordResolution,
    pub metadata: Option<ResponseMeta>,
}

/// The named view selected for a known record.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum View {
    #[default]
    Record,
    Children,
    Grounding,
    Impact,
    Review,
}

impl View {
    pub const ALL: [Self; 5] = [
        Self::Record,
        Self::Children,
        Self::Grounding,
        Self::Impact,
        Self::Review,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Record => "record",
            Self::Children => "children",
            Self::Grounding => "grounding",
            Self::Impact => "impact",
            Self::Review => "review",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|view| view.as_str() == value)
    }
}

/// One semantic known-record request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetInput {
    #[schemars(length(min = 1))]
    pub target: String,
    #[serde(default)]
    pub view: View,
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub max_depth: Option<usize>,
    #[serde(default)]
    pub returned_kinds: Vec<NodeType>,
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub limit: Option<usize>,
    /// Registers a client thread for review changes, or ends its registration.
    #[serde(default)]
    pub notify: Option<Notify>,
}

impl GetInput {
    pub fn new(target: impl Into<String>, view: View) -> Self {
        Self {
            target: target.into(),
            view,
            max_depth: None,
            returned_kinds: Vec::new(),
            limit: None,
            notify: None,
        }
    }
}

/// The refusal message when no review host serves the repository and scope.
pub const NO_REVIEW_HOST: &str = "no review host is running for this repository and scope";

/// The client that gets review changes: `codex` registers a thread, `off` ends the
/// registration of that thread.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotifyClient {
    Codex,
    Off,
}

/// One notify request for a client thread.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Notify {
    pub client: NotifyClient,
    /// The Codex thread UUID.
    #[schemars(length(min = 1))]
    pub thread: String,
}

/// The result of one notify request. `accepted` is true when Codex accepted the full
/// document message into the thread queue. It does not show that the agent read it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub client: NotifyClient,
    pub thread: String,
    pub registered: bool,
    pub accepted: bool,
    #[serde(default)]
    pub removed: bool,
}

/// Sends a notify request to the review host of the repository and scope.
pub trait NotifyPort: Send + Sync {
    fn register<'a>(
        &'a self,
        root: &'a str,
        limit: usize,
        notify: &'a Notify,
    ) -> PortFuture<'a, Registration>;
}

/// The bound and continuation state of a potentially incomplete result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    #[schemars(range(min = 1))]
    pub limit: usize,
    #[schemars(range(min = 1))]
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
    pub shaping: Option<SessionSummary>,
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

/// Current review state and bounded Discussion conversations for one record.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct Review {
    pub edit: RequirementEditState,
    pub decision: RequirementDecisionState,
    pub update_precondition: String,
    pub discussions: DiscussionResultPage<DiscussionConversationResult>,
    pub follow_up_commands: Vec<String>,
    pub bounds: Bounds,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_metadata: Option<ResponseMeta>,
    /// The notify result. The get wire shows it beside the review, not in it.
    #[serde(skip)]
    #[schemars(skip)]
    pub notify: Option<Registration>,
}

/// The data selected by one named view.
#[derive(Clone, Debug)]
pub enum ViewResult {
    Record,
    Children(Traversal),
    Grounding(Traversal),
    Impact(Impact),
    Review(Box<Review>),
}

impl ViewResult {
    pub const fn view(&self) -> View {
        match self {
            Self::Record => View::Record,
            Self::Children(_) => View::Children,
            Self::Grounding(_) => View::Grounding,
            Self::Impact(_) => View::Impact,
            Self::Review(_) => View::Review,
        }
    }

    pub fn related(&self) -> &[TracedNode] {
        match self {
            Self::Children(traversal) | Self::Grounding(traversal) => &traversal.records,
            Self::Record | Self::Impact(_) | Self::Review(_) => &[],
        }
    }

    pub const fn impact(&self) -> Option<&ImpactResult> {
        match self {
            Self::Impact(impact) => Some(&impact.detail),
            _ => None,
        }
    }

    pub const fn review(&self) -> Option<&Review> {
        match self {
            Self::Review(review) => Some(review),
            _ => None,
        }
    }

    pub const fn bounds(&self) -> Option<&Bounds> {
        match self {
            Self::Children(traversal) | Self::Grounding(traversal) => Some(&traversal.bounds),
            Self::Impact(impact) => Some(&impact.bounds),
            Self::Review(review) => Some(&review.bounds),
            Self::Record => None,
        }
    }

    pub const fn metadata(&self) -> Option<&ResponseMeta> {
        match self {
            Self::Children(traversal) | Self::Grounding(traversal) => {
                traversal.response_metadata.as_ref()
            }
            Self::Impact(impact) => impact.response_metadata.as_ref(),
            Self::Review(review) => review.response_metadata.as_ref(),
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

    pub const fn review(&self) -> Option<&Review> {
        self.result.review()
    }

    pub const fn bounds(&self) -> Option<&Bounds> {
        self.result.bounds()
    }

    pub const fn shaping(&self) -> Option<&SessionSummary> {
        match &self.result {
            ViewResult::Children(traversal) => traversal.shaping.as_ref(),
            _ => None,
        }
    }

    pub const fn view_metadata(&self) -> Option<&ResponseMeta> {
        self.result.metadata()
    }
}

/// A failure from validation, identity resolution, or an injected operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadError {
    InvalidOptions,
    NotFound,
    AmbiguousIdentity,
    /// The review host refused one request field.
    InvalidInput {
        field: String,
        message: String,
    },
    /// The review host could not finish the request now; the same request can run again.
    HostBusy(String),
    Operation(String),
}

impl Display for ReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOptions => formatter.write_str("unsupported read options"),
            Self::NotFound => formatter.write_str("record does not exist"),
            Self::AmbiguousIdentity => formatter.write_str("record ID is not unique"),
            Self::InvalidInput { message, .. }
            | Self::HostBusy(message)
            | Self::Operation(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ReadError {}

impl ReadError {
    /// The `{error, meta}` object that the CLI and MCP both return for this failure.
    #[rule("rule_mcp_get_failure_matches_cli")]
    pub fn failure(&self) -> serde_json::Value {
        let mut error = match self {
            Self::InvalidOptions => {
                serde_json::json!({"kind": "invalid_input", "reason": "invalid_value"})
            }
            Self::NotFound => serde_json::json!({"kind": "resource_not_found"}),
            Self::AmbiguousIdentity => serde_json::json!({
                "kind": "invalid_input", "reason": "invalid_value", "field": "target"
            }),
            Self::InvalidInput { field, .. } => serde_json::json!({
                "kind": "invalid_input", "reason": "invalid_value", "field": field
            }),
            Self::HostBusy(_) => serde_json::json!({"kind": "host_busy"}),
            Self::Operation(_) => serde_json::json!({"kind": "operation_failed"}),
        };
        error["message"] = serde_json::Value::String(self.to_string());
        serde_json::json!({"error": error, "meta": {}})
    }
}

/// Existing operation paths needed to compose a known-record read.
pub trait GetPort: Send + Sync {
    fn resolve<'a>(&'a self, id: &'a str) -> PortFuture<'a, RecordResolution>;
    fn traverse(&self, request: TraversalRequest) -> PortFuture<'_, Traversal>;
    fn impact<'a>(&'a self, record: &'a GraphNode, limit: usize) -> PortFuture<'a, Impact>;
    fn review<'a>(&'a self, _record: &'a GraphNode, _limit: usize) -> PortFuture<'a, Review> {
        Box::pin(async { Err(ReadError::InvalidOptions) })
    }
    fn notify<'a>(
        &'a self,
        _root: &'a str,
        _limit: usize,
        _notify: &'a Notify,
    ) -> PortFuture<'a, Registration> {
        Box::pin(async { Err(ReadError::Operation(NO_REVIEW_HOST.to_owned())) })
    }
}

impl<P: GetPort> crate::Porcelain<P> {
    /// Resolves one record and composes its selected human-facing view.
    #[rule("rule_porcelain_id_needs_no_kind_selector")]
    #[rule("rule_porcelain_get_returns_record")]
    #[rule("rule_porcelain_get_selects_child_context")]
    #[rule("rule_porcelain_return_filter_keeps_traversal")]
    #[rule("rule_porcelain_get_has_grounding_impact")]
    #[rule("rule_porcelain_read_rejects_bad_options")]
    #[rule("rule_review_notify_cli_mcp_same")]
    #[rule("rule_shaping_recovery_bounds")]
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
                if let Some(summary) = &mut traversal.shaping {
                    if !input.returned_kinds.is_empty()
                        || input.max_depth.is_some()
                        || record_metadata.as_ref().is_none_or(|metadata| {
                            metadata.freshness_error.is_some()
                                || !same_revision(metadata, traversal.response_metadata.as_ref())
                        })
                    {
                        summary.complete = false;
                    }
                }
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
            View::Review => {
                let limit = input.limit.unwrap_or(50);
                let mut review = self.port.review(&record, limit).await?;
                if let Some(notify) = &input.notify {
                    review.notify = Some(
                        self.port
                            .notify(record.id().as_str(), limit, notify)
                            .await?,
                    );
                }
                ViewResult::Review(Box::new(review))
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
    if input.target.is_empty() {
        return Err(ReadError::InvalidOptions);
    }
    let has_traversal_options = input.max_depth.is_some() || !input.returned_kinds.is_empty();
    let unsupported = match input.view {
        View::Record => has_traversal_options || input.limit.is_some(),
        View::Children | View::Grounding => false,
        View::Impact | View::Review => has_traversal_options,
    };
    let notify_refused = input
        .notify
        .as_ref()
        .is_some_and(|notify| input.view != View::Review || notify.thread.is_empty());
    if unsupported || notify_refused || input.max_depth == Some(0) || input.limit == Some(0) {
        return Err(ReadError::InvalidOptions);
    }
    Ok(())
}

fn same_revision(left: &ResponseMeta, right: Option<&ResponseMeta>) -> bool {
    left.stamp
        .as_ref()
        .zip(right.and_then(|metadata| metadata.stamp.as_ref()))
        .is_some_and(|(left, right)| {
            left.instance_id == right.instance_id
                && left.serial == right.serial
                && left.digest == right.digest
        })
}
