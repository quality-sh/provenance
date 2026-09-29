use super::{review_reads::ReadResult, shapes::graph_read_operation, ExecutionNeed};
use provenance_core::threads::{
    DiscussionConversationQuery, DiscussionConversationResult, DiscussionListQuery,
    DiscussionResultPage, DiscussionSummary,
};

graph_read_operation!(
    pub ListDiscussions,
    "discussion-list",
    DiscussionListQuery,
    ReadResult<DiscussionResultPage<DiscussionSummary>>,
    &[409],
    |_| {
        &[
            ExecutionNeed::GraphStorage,
            ExecutionNeed::ProjectionMaintenance,
        ]
    },
    |read, request| async move {
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
    }
);

graph_read_operation!(
    pub GetDiscussionConversation,
    "discussion-conversation",
    DiscussionConversationQuery,
    ReadResult<DiscussionConversationResult>,
    &[409],
    |_| {
        &[
            ExecutionNeed::GraphStorage,
            ExecutionNeed::ProjectionMaintenance,
        ]
    },
    |read, request| async move {
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
    }
);
