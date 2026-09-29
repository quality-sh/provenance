//! Addressed Discussion reads that do not page through collection results.

use super::{review_reads::ReadResult, shapes::graph_read_operation, ExecutionNeed};
use provenance_core::{threads::DiscussionGroup, StableId, ThreadParent};
use serde::Deserialize;

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DiscussionRequest {
    pub parent: ThreadParent,
    pub discussion_id: StableId,
}

graph_read_operation!(
    pub ReviewDiscussion,
    "review-discussion",
    DiscussionRequest,
    ReadResult<DiscussionGroup>,
    &[409],
    |_| {
        &[
            ExecutionNeed::GraphStorage,
            ExecutionNeed::ProjectionMaintenance,
        ]
    },
    |read, request| async move {
        Ok(crate::review::read_discussion(
            &read.root,
            &read.scope,
            read.policy,
            request.parent,
            request.discussion_id,
        )
        .await?
        .into())
    }
);
