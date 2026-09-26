//! Addressed Discussion reads that do not page through collection results.

use super::{
    failures::ReadError, v2_review_reads::ReadResult, ContextKind, ExecutionNeed, ExecutionNeeds,
    Operation, OperationFuture, PreparedContext,
};
use provenance_core::{threads::DiscussionGroup, StableId, ThreadParent};
use serde::Deserialize;

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DiscussionRequest {
    pub parent: ThreadParent,
    pub discussion_id: StableId,
}

pub struct ReviewDiscussionV2;
impl Operation for ReviewDiscussionV2 {
    type Request = DiscussionRequest;
    type Success = ReadResult<DiscussionGroup>;
    type Failure = ReadError;
    const NAME: &'static str = "review-discussion-v2";
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
            Ok(crate::review::read_discussion(
                &read.root,
                &read.scope,
                read.policy,
                request.parent,
                request.discussion_id,
            )
            .await?
            .into())
        })
    }
}
