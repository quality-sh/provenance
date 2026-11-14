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
use std::{future::Future, pin::Pin};

mod notify;
mod read_error;
mod review_link;
mod shaping;
mod wire;
pub use notify::{Notify, NotifyClient, NotifyPort, Registration, NO_REVIEW_HOST};
pub use read_error::ReadError;
pub use review_link::{ReviewLink, ReviewLinkPort};
pub use shaping::{BlockedQuestion, BlockedReason, Claim, QuestionState, SessionSummary};
pub use wire::{
    input_schema, notify_input_schema, notify_output_schema, output_schema, render_readable,
    trace_input_schema, trace_output_schema, NotifyInput, NotifyResult, TraceInput,
};

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
    Trace,
    Impact,
    Review,
}

impl View {
    pub const ALL: [Self; 5] = [
        Self::Record,
        Self::Children,
        Self::Trace,
        Self::Impact,
        Self::Review,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Record => "record",
            Self::Children => "children",
            Self::Trace => "trace",
            Self::Impact => "impact",
            Self::Review => "review",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|view| view.as_str() == value)
    }
}

/// One semantic known-record request: the target and the get fields.
// Each bool is one switch field.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetInput {
    #[schemars(length(min = 1))]
    pub target: String,
    #[serde(default)]
    pub view: View,
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub depth: Option<usize>,
    #[serde(default)]
    pub kind: Vec<NodeType>,
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub limit: Option<usize>,
    #[serde(default)]
    pub review_link: bool,
    #[serde(default)]
    pub notify: Option<NotifyClient>,
    #[serde(default)]
    #[schemars(length(min = 1))]
    pub thread: Option<String>,
}

impl GetInput {
    pub fn new(target: impl Into<String>, view: View) -> Self {
        Self {
            target: target.into(),
            view,
            depth: None,
            kind: Vec::new(),
            limit: None,
            review_link: false,
            notify: None,
            thread: None,
        }
    }

    /// The notify request of the `notify` and `thread` fields.
    pub fn notify_request(&self) -> Option<Notify> {
        self.notify.map(|client| Notify {
            client,
            thread: self.thread.clone().unwrap_or_default(),
        })
    }
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
    /// The record view: the edit state that the next guarded update of the record needs,
    /// and the review link of the record when the request asks for it.
    Record {
        edit: RequirementEditState,
        review_link: Option<ReviewLink>,
    },
    Children(Traversal),
    Trace(Traversal),
    Impact(Impact),
    Review(Box<Review>),
}

impl ViewResult {
    pub const fn view(&self) -> View {
        match self {
            Self::Record { .. } => View::Record,
            Self::Children(_) => View::Children,
            Self::Trace(_) => View::Trace,
            Self::Impact(_) => View::Impact,
            Self::Review(_) => View::Review,
        }
    }

    pub fn related(&self) -> &[TracedNode] {
        match self {
            Self::Children(traversal) | Self::Trace(traversal) => &traversal.records,
            Self::Record { .. } | Self::Impact(_) | Self::Review(_) => &[],
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

    pub const fn edit(&self) -> Option<&RequirementEditState> {
        match self {
            Self::Record { edit, .. } => Some(edit),
            _ => None,
        }
    }

    pub const fn review_link(&self) -> Option<&ReviewLink> {
        match self {
            Self::Record {
                review_link: Some(link),
                ..
            } => Some(link),
            _ => None,
        }
    }

    pub const fn bounds(&self) -> Option<&Bounds> {
        match self {
            Self::Children(traversal) | Self::Trace(traversal) => Some(&traversal.bounds),
            Self::Impact(impact) => Some(&impact.bounds),
            Self::Review(review) => Some(&review.bounds),
            Self::Record { .. } => None,
        }
    }

    pub const fn metadata(&self) -> Option<&ResponseMeta> {
        match self {
            Self::Children(traversal) | Self::Trace(traversal) => {
                traversal.response_metadata.as_ref()
            }
            Self::Impact(impact) => impact.response_metadata.as_ref(),
            Self::Review(review) => review.response_metadata.as_ref(),
            Self::Record { .. } => None,
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

    /// The edit state of the record. Only the record view has it.
    pub const fn edit(&self) -> Option<&RequirementEditState> {
        self.result.edit()
    }

    pub const fn review_link(&self) -> Option<&ReviewLink> {
        self.result.review_link()
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

/// Existing operation paths needed to compose a known-record read.
pub trait GetPort: Send + Sync {
    fn resolve<'a>(&'a self, id: &'a str) -> PortFuture<'a, RecordResolution>;
    fn traverse(&self, request: TraversalRequest) -> PortFuture<'_, Traversal>;
    fn impact<'a>(&'a self, record: &'a GraphNode, limit: usize) -> PortFuture<'a, Impact>;
    /// The edit state of a resolved record. It comes from the resolved record, so it
    /// agrees with the record that the get shows.
    fn edit(&self, record: &GraphNode) -> Result<RequirementEditState, ReadError>;
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
    fn review_link<'a>(&'a self, _record: &'a GraphNode) -> PortFuture<'a, ReviewLink> {
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
            View::Record => self.record_view(&record, input.review_link).await?,
            View::Children | View::Trace => {
                let mut traversal = self
                    .port
                    .traverse(TraversalRequest {
                        target: input.target,
                        kind: record.node_type(),
                        view: input.view,
                        max_depth: input.depth.unwrap_or(1),
                        limit: input.limit.unwrap_or(50),
                    })
                    .await?;
                if let Some(summary) = &mut traversal.shaping {
                    if !input.kind.is_empty()
                        || input.depth.is_some()
                        || record_metadata.as_ref().is_none_or(|metadata| {
                            metadata.freshness_error.is_some()
                                || !same_revision(metadata, traversal.response_metadata.as_ref())
                        })
                    {
                        summary.complete = false;
                    }
                }
                traversal.records.retain(|candidate| {
                    input.kind.is_empty() || input.kind.contains(&candidate.node.node_type())
                });
                if input.view == View::Children {
                    ViewResult::Children(traversal)
                } else {
                    ViewResult::Trace(traversal)
                }
            }
            View::Impact => {
                ViewResult::Impact(self.port.impact(&record, input.limit.unwrap_or(50)).await?)
            }
            View::Review => {
                let limit = input.limit.unwrap_or(50);
                let mut review = self.port.review(&record, limit).await?;
                if let Some(notify) = input.notify_request() {
                    review.notify = Some(
                        self.port
                            .notify(record.id().as_str(), limit, &notify)
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

impl<P: GetPort> crate::Porcelain<P> {
    /// The record view gives the edit state of the record, so that the next guarded
    /// update of the record needs no other read.
    #[rule("rule_get_record_gives_edit_etag")]
    async fn record_view(
        &self,
        record: &GraphNode,
        review_link: bool,
    ) -> Result<ViewResult, ReadError> {
        let edit = self.port.edit(record)?;
        let review_link = if review_link {
            Some(self.port.review_link(record).await?)
        } else {
            None
        };
        Ok(ViewResult::Record { edit, review_link })
    }

    /// Walks upward from one record to its Sources: the get trace view without a view
    /// selector.
    #[rule("rule_trace_action_matches_trace_view")]
    pub async fn trace(&self, input: TraceInput) -> Result<GetOutcome, ReadError> {
        self.get(input.into()).await
    }
}

fn validate(input: &GetInput) -> Result<(), ReadError> {
    if input.target.is_empty() {
        return Err(ReadError::InvalidOptions);
    }
    let has_traversal_options = input.depth.is_some() || !input.kind.is_empty();
    let unsupported = match input.view {
        View::Record => has_traversal_options || input.limit.is_some(),
        View::Children | View::Trace => false,
        View::Impact | View::Review => has_traversal_options,
    };
    let notify_refused = match input.notify {
        Some(_) => input.view != View::Review || input.thread.as_deref().is_none_or(str::is_empty),
        None => input.thread.is_some(),
    };
    let review_link_refused = input.review_link
        && (input.view != View::Record || input.limit.is_some() || input.notify.is_some());
    if unsupported
        || notify_refused
        || review_link_refused
        || input.depth == Some(0)
        || input.limit == Some(0)
    {
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
