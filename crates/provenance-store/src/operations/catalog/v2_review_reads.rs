//! Resource adapters for review history and addressed Discussions.
use super::{
    failures::ReadError, ContextKind, ExecutionNeed, ExecutionNeeds, Operation, OperationFuture,
    PreparedContext,
};
use crate::{
    layout::ProvenanceLayout, operations::read_policy::ReadPolicy, review, state_store::StateStore,
    write_error::WriteError,
};
use provenance_core::{
    protocol::failure::{InvalidInputReason, OperationFailure},
    review::{EvidencePage, EvidenceQuery, ReviewEntry, ReviewHistoryQuery},
    threads::{
        DiscussionEntry, DiscussionGroup, DiscussionMessagesQuery, DiscussionQuery,
        DiscussionSelector,
    },
    Message, ScopeId, StableId, ThreadParent,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ReadResult<T> {
    pub result: T,
    pub stamp: provenance_core::protocol::Stamp,
    pub freshness_error: Option<String>,
}

#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct DiscussionResultPage<T> {
    pub entries: Vec<T>,
    pub limit: usize,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

const fn discussion_result<T>(
    entries: Vec<T>,
    next_cursor: Option<String>,
    limit: usize,
) -> DiscussionResultPage<T> {
    DiscussionResultPage {
        entries,
        limit,
        has_more: next_cursor.is_some(),
        next_cursor,
    }
}
impl<T> From<provenance_core::protocol::Stamped<T>> for ReadResult<T> {
    fn from(value: provenance_core::protocol::Stamped<T>) -> Self {
        Self {
            result: value.result,
            stamp: value.stamp,
            freshness_error: value.freshness_error,
        }
    }
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct StartDiscussionData {
    pub actor: String,
    pub declared_by: Option<String>,
    pub role: provenance_core::MessageRole,
    pub body: String,
}
#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ReplyDiscussionData {
    pub actor: String,
    pub declared_by: Option<String>,
    pub role: provenance_core::MessageRole,
    pub body: String,
}
#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UpdateDiscussionData {
    pub actor: String,
    pub declared_by: Option<String>,
    pub status: provenance_core::threads::DiscussionStatus,
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HistoryRequest {
    pub requirement_id: StableId,
    #[serde(default = "limit")]
    pub limit: usize,
    pub cursor: Option<String>,
}
const fn limit() -> usize {
    50
}

pub struct ReviewHistoryV2;
impl Operation for ReviewHistoryV2 {
    type Request = HistoryRequest;
    type Success = ReadResult<DiscussionResultPage<ReviewEntry>>;
    type Failure = ReadError;
    const NAME: &'static str = "review-history-v2";
    const CONTEXT: ContextKind = ContextKind::Scoped;
    const FAILURE_STATUSES: &'static [u16] = &[409];
    fn needs(_: &Self::Request) -> ExecutionNeeds {
        &[
            ExecutionNeed::GraphStorage,
            ExecutionNeed::ProjectionMaintenance,
        ]
    }
    fn failure_status(error: &ReadError) -> u16 {
        error.status()
    }
    fn run(
        context: PreparedContext,
        request: Self::Request,
    ) -> OperationFuture<Self::Success, Self::Failure> {
        Box::pin(async move {
            let read = context.graph()?;
            let limit = request.limit;
            let page = review::read_history(
                &read.root,
                &read.scope,
                read.policy,
                ReviewHistoryQuery {
                    requirement_id: request.requirement_id,
                    limit: request.limit,
                    cursor: request.cursor,
                },
            )
            .await?;
            Ok(ReadResult {
                result: discussion_result(page.result.entries, page.result.next_cursor, limit),
                stamp: page.stamp,
                freshness_error: page.freshness_error,
            })
        })
    }
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HistoryEntryRequest {
    pub requirement_id: StableId,
    pub entry_id: StableId,
}

pub struct ReviewHistoryEntryV2;
impl Operation for ReviewHistoryEntryV2 {
    type Request = HistoryEntryRequest;
    type Success = ReviewEntry;
    type Failure = ReadError;
    const NAME: &'static str = "review-history-entry-v2";
    const CONTEXT: ContextKind = ContextKind::Scoped;
    const FAILURE_STATUSES: &'static [u16] = &[409];
    fn needs(_: &Self::Request) -> ExecutionNeeds {
        &[
            ExecutionNeed::GraphStorage,
            ExecutionNeed::ProjectionMaintenance,
        ]
    }
    fn failure_status(error: &ReadError) -> u16 {
        error.status()
    }
    fn run(
        context: PreparedContext,
        request: Self::Request,
    ) -> OperationFuture<Self::Success, Self::Failure> {
        Box::pin(async move {
            let read = context.graph()?;
            let mut cursor = None;
            loop {
                let page = review::read_history(
                    &read.root,
                    &read.scope,
                    read.policy,
                    ReviewHistoryQuery {
                        requirement_id: request.requirement_id.clone(),
                        limit: 200,
                        cursor,
                    },
                )
                .await?;
                if let Some(entry) = page
                    .result
                    .entries
                    .into_iter()
                    .find(|entry| entry.id == request.entry_id)
                {
                    return Ok(entry);
                }
                match page.result.next_cursor {
                    Some(next) => cursor = Some(next),
                    None => {
                        return Err(anyhow::Error::new(
                            provenance_core::protocol::read_failure::ReadFailure::ResourceNotFound,
                        )
                        .into())
                    }
                }
            }
        })
    }
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HistoryEvidenceRequest {
    pub requirement_id: StableId,
    pub entry_id: StableId,
    pub side: String,
    pub field: Option<String>,
    #[serde(default)]
    pub offset: u64,
}

pub struct ReviewEvidenceV2;
impl Operation for ReviewEvidenceV2 {
    type Request = HistoryEvidenceRequest;
    type Success = ReadResult<EvidencePage>;
    type Failure = ReadError;
    const NAME: &'static str = "review-evidence-v2";
    const CONTEXT: ContextKind = ContextKind::Scoped;
    const FAILURE_STATUSES: &'static [u16] = &[409];
    fn validate_external(request: &Self::Request) -> Result<(), OperationFailure> {
        if matches!(request.side.as_str(), "before" | "after") {
            Ok(())
        } else {
            Err(OperationFailure::InvalidInput {
                field: Some("side".into()),
                reason: InvalidInputReason::InvalidValue,
            })
        }
    }
    fn needs(_: &Self::Request) -> ExecutionNeeds {
        &[
            ExecutionNeed::GraphStorage,
            ExecutionNeed::ProjectionMaintenance,
        ]
    }
    fn failure_status(error: &ReadError) -> u16 {
        error.status()
    }
    fn run(
        context: PreparedContext,
        request: Self::Request,
    ) -> OperationFuture<Self::Success, Self::Failure> {
        Box::pin(async move {
            let read = context.graph()?;
            Ok(review::read_evidence(
                &read.root,
                &read.scope,
                read.policy,
                EvidenceQuery {
                    requirement_id: request.requirement_id,
                    entry_id: request.entry_id,
                    before: request.side == "before",
                    field: request.field,
                    offset: request.offset,
                },
            )
            .await?
            .into())
        })
    }
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DiscussionsRequest {
    pub parent: ThreadParent,
    #[serde(default = "limit")]
    pub limit: usize,
    pub cursor: Option<String>,
}

pub struct ReviewDiscussionsV2;
impl Operation for ReviewDiscussionsV2 {
    type Request = DiscussionsRequest;
    type Success = ReadResult<DiscussionResultPage<DiscussionGroup>>;
    type Failure = ReadError;
    const NAME: &'static str = "review-discussions-v2";
    const CONTEXT: ContextKind = ContextKind::Scoped;
    const FAILURE_STATUSES: &'static [u16] = &[409];
    fn needs(_: &Self::Request) -> ExecutionNeeds {
        &[
            ExecutionNeed::GraphStorage,
            ExecutionNeed::ProjectionMaintenance,
        ]
    }
    fn failure_status(error: &ReadError) -> u16 {
        error.status()
    }
    fn run(
        context: PreparedContext,
        request: Self::Request,
    ) -> OperationFuture<Self::Success, Self::Failure> {
        Box::pin(async move {
            let read = context.graph()?;
            let limit = request.limit;
            let page = review::read_discussions(
                &read.root,
                &read.scope,
                read.policy,
                DiscussionQuery {
                    parent: request.parent,
                    limit: request.limit,
                    cursor: request.cursor,
                },
            )
            .await?;
            Ok(ReadResult {
                result: discussion_result(page.result.entries, page.result.next_cursor, limit),
                stamp: page.stamp,
                freshness_error: page.freshness_error,
            })
        })
    }
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DiscussionMessagesRequest {
    pub parent: ThreadParent,
    pub selector: DiscussionSelector,
    #[serde(default = "limit")]
    pub limit: usize,
    pub cursor: Option<String>,
}

pub struct ReviewDiscussionMessagesV2;
impl Operation for ReviewDiscussionMessagesV2 {
    type Request = DiscussionMessagesRequest;
    type Success = ReadResult<DiscussionResultPage<Message>>;
    type Failure = ReadError;
    const NAME: &'static str = "review-discussion-messages-v2";
    const CONTEXT: ContextKind = ContextKind::Scoped;
    const FAILURE_STATUSES: &'static [u16] = &[409];
    fn needs(_: &Self::Request) -> ExecutionNeeds {
        &[
            ExecutionNeed::GraphStorage,
            ExecutionNeed::ProjectionMaintenance,
        ]
    }
    fn failure_status(error: &ReadError) -> u16 {
        error.status()
    }
    fn run(
        context: PreparedContext,
        request: Self::Request,
    ) -> OperationFuture<Self::Success, Self::Failure> {
        Box::pin(async move {
            let read = context.graph()?;
            let limit = request.limit;
            let page = review::read_discussion_messages(
                &read.root,
                &read.scope,
                read.policy,
                DiscussionMessagesQuery {
                    parent: request.parent,
                    selector: request.selector,
                    limit: request.limit,
                    cursor: request.cursor,
                },
            )
            .await?;
            Ok(ReadResult {
                result: discussion_result(page.result.entries, page.result.next_cursor, limit),
                stamp: page.stamp,
                freshness_error: page.freshness_error,
            })
        })
    }
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DiscussionMessageRequest {
    pub parent: ThreadParent,
    pub selector: DiscussionSelector,
    pub message_id: StableId,
}

pub struct ReviewDiscussionMessageV2;
impl Operation for ReviewDiscussionMessageV2 {
    type Request = DiscussionMessageRequest;
    type Success = ReadResult<Message>;
    type Failure = ReadError;
    const NAME: &'static str = "review-discussion-message-v2";
    const CONTEXT: ContextKind = ContextKind::Scoped;
    const FAILURE_STATUSES: &'static [u16] = &[409];
    fn needs(_: &Self::Request) -> ExecutionNeeds {
        &[
            ExecutionNeed::GraphStorage,
            ExecutionNeed::ProjectionMaintenance,
        ]
    }
    fn failure_status(error: &ReadError) -> u16 {
        error.status()
    }
    fn run(
        context: PreparedContext,
        request: Self::Request,
    ) -> OperationFuture<Self::Success, Self::Failure> {
        Box::pin(async move {
            let read = context.graph()?;
            Ok(review::read_discussion_message(
                &read.root,
                &read.scope,
                read.policy,
                request.parent,
                request.selector,
                request.message_id,
            )
            .await?
            .into())
        })
    }
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WriteDiscussionRequest {
    pub scope_id: ScopeId,
    pub parent: ThreadParent,
    pub request_id: StableId,
    pub actor: String,
    pub declared_by: Option<String>,
    pub action: review::DiscussionAction,
}

pub struct WriteDiscussionV2;
impl Operation for WriteDiscussionV2 {
    type Request = WriteDiscussionRequest;
    type Success = DiscussionEntry;
    type Failure = WriteError;
    const NAME: &'static str = "write-discussion-v2";
    const MUTATES: bool = true;
    const CONTEXT: ContextKind = ContextKind::Scope;
    const FAILURE_STATUSES: &'static [u16] = &[409];
    fn needs(_: &Self::Request) -> ExecutionNeeds {
        &[ExecutionNeed::GraphStorage]
    }
    fn failure_status(error: &WriteError) -> u16 {
        error.status()
    }
    fn run(
        context: PreparedContext,
        request: Self::Request,
    ) -> OperationFuture<Self::Success, Self::Failure> {
        Box::pin(async move {
            let context = context.scope()?;
            if request.scope_id != context.scope {
                return Err(anyhow::anyhow!("request scope does not match selected scope").into());
            }
            StateStore::new(ProvenanceLayout::new(context.root))
                .write_discussion(review::WriteDiscussion {
                    scope_id: request.scope_id,
                    parent: request.parent,
                    request_id: request.request_id,
                    actor: request.actor,
                    declared_by: request.declared_by,
                    action: request.action,
                })
                .map_err(Into::into)
        })
    }
}

pub fn parent(kind: &str, id: StableId) -> anyhow::Result<ThreadParent> {
    Ok(ThreadParent {
        node_type: serde_json::from_value(serde_json::Value::String(kind.to_owned()))?,
        node_id: id,
    })
}

#[allow(dead_code)]
fn keep_types(_: (DiscussionGroup, Message, ReadPolicy)) {}
