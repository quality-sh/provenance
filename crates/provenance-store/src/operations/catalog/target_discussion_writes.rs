use super::{shapes::scoped_write_operation, ExecutionNeed};
use crate::review;
use provenance_core::threads::DiscussionEntry;

scoped_write_operation!(
    pub WriteTargetDiscussion,
    "write-target-discussion",
    review::TargetDiscussionWrite,
    DiscussionEntry,
    &[409],
    &[ExecutionNeed::GraphStorage],
    scope = scope_id,
    |store, _scope, request| store.write_target_discussion(request)
);
