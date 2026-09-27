//! Scope-bound Discussion writes.

use super::{shapes::scoped_write_operation, ExecutionNeed};
use crate::review;
use provenance_core::{threads::DiscussionEntry, ScopeId, StableId, ThreadParent};
use serde::Deserialize;

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

scoped_write_operation!(
    pub WriteDiscussionV2,
    "write-discussion-v2",
    WriteDiscussionRequest,
    DiscussionEntry,
    &[409],
    &[ExecutionNeed::GraphStorage],
    scope = scope_id,
    |store, _scope, request| store.write_discussion(review::WriteDiscussion {
        scope_id: request.scope_id,
        parent: request.parent,
        request_id: request.request_id,
        actor: request.actor,
        declared_by: request.declared_by,
        action: request.action,
    })
);
