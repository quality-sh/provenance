//! Resource adapters for review history and addressed Discussions.
use super::{shapes::graph_read_operation, ExecutionNeed};
use crate::review;
pub use provenance_core::threads::DiscussionResultPage;
use provenance_core::{
    review::{EvidencePage, EvidenceQuery, ReviewEntry, ReviewHistoryQuery},
    threads::{DiscussionGroup, DiscussionMessagesQuery, DiscussionQuery, DiscussionSelector},
    Message, StableId, ThreadParent,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ReadResult<T> {
    pub result: T,
    pub stamp: provenance_core::protocol::Stamp,
    pub freshness_error: Option<String>,
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

macro_rules! review_read {
    ($name:ident, $wire:literal, $request:ty, $success:ty,
     |$read:ident, $input:ident| $body:expr) => {
        graph_read_operation!(
            pub $name,
            $wire,
            $request,
            $success,
            &[409],
            |_| &[
                ExecutionNeed::GraphStorage,
                ExecutionNeed::ProjectionMaintenance,
            ],
            |$read, $input| $body
        );
    };
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

review_read!(
    ReviewHistoryV2,
    "review-history-v2",
    HistoryRequest,
    ReadResult<DiscussionResultPage<ReviewEntry>>,
    |read, request| async move {
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
    }
);

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HistoryEntryRequest {
    pub requirement_id: StableId,
    pub entry_id: StableId,
}

review_read!(
    ReviewHistoryEntryV2,
    "review-history-entry-v2",
    HistoryEntryRequest,
    ReviewEntry,
    |read, request| async move {
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
    }
);

/// The published evidence side of a review outcome. The enum is the whole
/// contract: wire values outside `before` and `after` are unrepresentable, so
/// the request carries no second string validation.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewEvidenceSide {
    Before,
    After,
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct HistoryEvidenceRequest {
    pub requirement_id: StableId,
    pub entry_id: StableId,
    pub side: ReviewEvidenceSide,
    pub field: Option<String>,
    #[serde(default)]
    pub offset: u64,
}

review_read!(
    ReviewEvidenceV2,
    "review-evidence-v2",
    HistoryEvidenceRequest,
    ReadResult<EvidencePage>,
    |read, request| async move {
        Ok(review::read_evidence(
            &read.root,
            &read.scope,
            read.policy,
            EvidenceQuery {
                requirement_id: request.requirement_id,
                entry_id: request.entry_id,
                before: request.side == ReviewEvidenceSide::Before,
                field: request.field,
                offset: request.offset,
            },
        )
        .await?
        .into())
    }
);

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DiscussionsRequest {
    pub parent: ThreadParent,
    #[serde(default = "limit")]
    pub limit: usize,
    pub cursor: Option<String>,
}

review_read!(
    ReviewDiscussionsV2,
    "review-discussions-v2",
    DiscussionsRequest,
    ReadResult<DiscussionResultPage<DiscussionGroup>>,
    |read, request| async move {
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
    }
);

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

review_read!(
    ReviewDiscussionMessagesV2,
    "review-discussion-messages-v2",
    DiscussionMessagesRequest,
    ReadResult<DiscussionResultPage<Message>>,
    |read, request| async move {
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
    }
);

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DiscussionMessageRequest {
    pub parent: ThreadParent,
    pub selector: DiscussionSelector,
    pub message_id: StableId,
}

review_read!(
    ReviewDiscussionMessageV2,
    "review-discussion-message-v2",
    DiscussionMessageRequest,
    ReadResult<Message>,
    |read, request| async move {
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
    }
);

pub fn parent(kind: &str, id: StableId) -> anyhow::Result<ThreadParent> {
    Ok(ThreadParent {
        node_type: serde_json::from_value(serde_json::Value::String(kind.to_owned()))?,
        node_id: id,
    })
}
