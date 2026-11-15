use super::{shapes::scoped_write_operation, ExecutionNeed};
use crate::review;
use provenance_core::{threads::Discussion, MessageRole, NodeType, ScopeId, StableId};
use serde::Deserialize;

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WriteTargetDiscussionRequest {
    pub scope_id: ScopeId,
    #[serde(
        default,
        deserialize_with = "provenance_core::actor_input::given",
        skip_serializing_if = "Option::is_none"
    )]
    #[cfg_attr(
        feature = "schema",
        schemars(with = "String", description = provenance_core::actor_input::DESCRIPTION)
    )]
    pub actor: Option<String>,
    pub declared_by: Option<String>,
    pub allowed_parent_kinds: Vec<NodeType>,
    pub discussion_id: StableId,
    pub expected_version: u64,
    pub role: MessageRole,
    pub body: String,
}

scoped_write_operation!(
    pub WriteTargetDiscussion,
    "write-target-discussion",
    WriteTargetDiscussionRequest,
    Discussion,
    &[409],
    &[ExecutionNeed::GraphStorage],
    scope = scope_id,
    |store, _scope, request| store.write_target_discussion(review::TargetDiscussionWrite {
        scope_id: request.scope_id,
        actor: review::write_actor::recorded_actor(request.actor),
        declared_by: request.declared_by,
        allowed_parent_kinds: request.allowed_parent_kinds,
        discussion_id: request.discussion_id,
        expected_version: request.expected_version,
        role: request.role,
        body: request.body,
    })
);
