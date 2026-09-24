use super::{
    failures::ReadError,
    v2_review_reads::ReadResult,
    ContextKind, ExecutionNeed, ExecutionNeeds, Operation, OperationFuture, PreparedContext,
};
use provenance_core::{
    threads::{
        DiscussionConversationQuery, DiscussionConversationResult, DiscussionListQuery,
        DiscussionResultPage, DiscussionSummary,
    },
};

pub struct ListDiscussionsV2;

impl Operation for ListDiscussionsV2 {
    type Request = DiscussionListQuery;
    type Success = ReadResult<DiscussionResultPage<DiscussionSummary>>;
    type Failure = ReadError;
    const NAME: &'static str = "discussion-list-v2";
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
            let page =
                crate::review::read_discussion_list(&read.root, &read.scope, read.policy, request)
                    .await?;
            let next_cursor = page.result.next_cursor;
            Ok(ReadResult {
                result: DiscussionResultPage {
                    entries: page.result.entries,
                    limit,
                    has_more: next_cursor.is_some(),
                    next_cursor,
                },
                stamp: page.stamp,
                freshness_error: page.freshness_error,
            })
        })
    }
}

pub struct GetDiscussionConversationV2;

impl Operation for GetDiscussionConversationV2 {
    type Request = DiscussionConversationQuery;
    type Success = ReadResult<DiscussionConversationResult>;
    type Failure = ReadError;
    const NAME: &'static str = "discussion-conversation-v2";
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
            let conversation = crate::review::read_discussion_conversation(
                &read.root,
                &read.scope,
                read.policy,
                request,
            )
            .await?;
            let next_cursor = conversation.result.messages.next_cursor;
            Ok(ReadResult {
                result: DiscussionConversationResult {
                    head: conversation.result.head,
                    messages: DiscussionResultPage {
                        entries: conversation.result.messages.entries,
                        limit,
                        has_more: next_cursor.is_some(),
                        next_cursor,
                    },
                },
                stamp: conversation.stamp,
                freshness_error: conversation.freshness_error,
            })
        })
    }
}
