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
    /// The actor of the write. A status change must give an actor. A start that gives no
    /// actor records the actor `authoring`, and a reply needs no actor.
    #[serde(default, deserialize_with = "provenance_core::actor_input::given")]
    #[cfg_attr(feature = "schema", schemars(with = "String"))]
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
    validate = |request: &WriteDiscussionRequest| match request.action {
        review::DiscussionAction::SetStatus { .. } if request.actor.is_none() => {
            Err(review::write_actor::missing_actor())
        }
        _ => Ok(()),
    },
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
