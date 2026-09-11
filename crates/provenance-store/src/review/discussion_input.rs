use provenance_core::{threads::DiscussionStatus, MessageRole, ScopeId, StableId, ThreadParent};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriteDiscussion {
    pub scope_id: ScopeId,
    pub parent: ThreadParent,
    pub request_id: StableId,
    pub actor: String,
    pub declared_by: Option<String>,
    pub action: DiscussionAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DiscussionAction {
    Start {
        role: MessageRole,
        body: String,
    },
    Reply {
        discussion_id: StableId,
        expected_version: u64,
        role: MessageRole,
        body: String,
    },
    SetStatus {
        discussion_id: StableId,
        expected_version: u64,
        status: DiscussionStatus,
    },
}
