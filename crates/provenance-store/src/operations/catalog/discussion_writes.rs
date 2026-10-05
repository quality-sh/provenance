//! Scope-bound Discussion writes.

use super::{shapes::scoped_write_operation, ExecutionNeed};
use crate::review;
use provenance_core::{threads::Discussion, ScopeId, ThreadParent};
use serde::Deserialize;

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[provenance_macros::verifies("rule_review_request_identity_server_created", construction)]
#[serde(deny_unknown_fields)]
pub struct WriteDiscussionRequest {
    pub scope_id: ScopeId,
    pub parent: ThreadParent,
    pub actor: String,
    pub declared_by: Option<String>,
    pub action: review::DiscussionAction,
}

scoped_write_operation!(
    pub WriteDiscussion,
    "write-discussion",
    WriteDiscussionRequest,
    Discussion,
    &[409],
    &[ExecutionNeed::GraphStorage],
    scope = scope_id,
    |store, _scope, request| store.write_discussion(review::WriteDiscussion {
        scope_id: request.scope_id,
        parent: request.parent,
        actor: request.actor,
        declared_by: request.declared_by,
        action: request.action,
    })
);
