//! Scope-bound Discussion writes.

use super::{shapes::scoped_write_operation, ExecutionNeed};
use crate::review;
use provenance_core::{threads::Discussion, ScopeId, ThreadParent};
use serde::Deserialize;

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WriteDiscussionRequest {
    pub scope_id: ScopeId,
    pub parent: ThreadParent,
    /// The actor of the write. A status change must give an actor. A start or a reply that
    /// gives no actor records the actor `authoring`.
    pub actor: Option<String>,
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
    |store, _scope, request| {
        let actor = match request.action {
            review::DiscussionAction::SetStatus { .. } => {
                review::write_actor::named_actor(request.actor)?
            }
            review::DiscussionAction::Start { .. } | review::DiscussionAction::Reply { .. } => {
                review::write_actor::recorded_actor(request.actor)
            }
        };
        store.write_discussion(review::WriteDiscussion {
            scope_id: request.scope_id,
            parent: request.parent,
            actor,
            declared_by: request.declared_by,
            action: request.action,
        })
    }
);
