use provenance_core::{
    protocol::{failure::OperationError, Stamped},
    threads::{DiscussionConversation, DiscussionConversationQuery, DiscussionEntry, DiscussionListPage, DiscussionListQuery},
    NodeType, ScopeId,
};
use provenance_porcelain::discussion::{
    ConversationInput, DiscussionAction, DiscussionError, DiscussionPort, ListInput, PortFuture, ReplyInput, StartInput,
};
use provenance_store::{operations::catalog::{self, Operation as _}, review::{TargetDiscussionAction, TargetDiscussionWrite}};

/// Bind shared Discussion actions to canonical typed operations and host grants.
#[derive(Clone)]
pub struct HostDiscussionPort {
    host: crate::StatementHost,
}

impl HostDiscussionPort {
    pub const fn new(host: crate::StatementHost) -> Self {
        Self { host }
    }

    fn permitted_parent_kinds<O: catalog::Operation>(&self) -> Vec<NodeType> {
        catalog::definitions().iter()
            .filter(|definition| definition.registration.handler.operation == O::NAME)
            .filter(|definition| self.host.advertises(definition.name))
            .filter_map(|definition| definition.registration.request.parent.as_ref())
            .filter_map(|parent| NodeType::parse(parent.kind).ok())
            .collect()
    }

    fn permitted_write_parent_kinds(&self, action: DiscussionAction) -> Vec<NodeType> {
        let suffix = match action {
            DiscussionAction::Discuss => "-create-discussion",
            DiscussionAction::Reply => "-create-discussion-message",
            _ => return Vec::new(),
        };
        catalog::definitions().iter()
            .filter(|definition| definition.registration.handler.operation == catalog::WriteDiscussionV2::NAME)
            .filter(|definition| definition.name.ends_with(suffix) && self.host.advertises(definition.name))
            .filter_map(|definition| definition.registration.request.parent.as_ref())
            .filter_map(|parent| NodeType::parse(parent.kind).ok())
            .collect()
    }

    fn scope(&self) -> Result<ScopeId, DiscussionError> {
        self.host.bound_identity()
            .map(|(_, scope)| scope)
            .ok_or(DiscussionError::AccessDenied)
            .and_then(|scope| ScopeId::new(scope).map_err(|_| DiscussionError::AccessDenied))
    }
}

impl DiscussionPort for HostDiscussionPort {
    fn list(&self, input: ListInput) -> PortFuture<'_, Stamped<DiscussionListPage>> {
        Box::pin(async move {
            let kinds = self.permitted_parent_kinds::<catalog::ReviewDiscussionsV2>();
            if kinds.is_empty() { return Err(DiscussionError::AccessDenied); }
            if input.parent.as_ref().is_some_and(|parent| !kinds.contains(&parent.node_type)) {
                return Err(DiscussionError::AccessDenied);
            }
            let response = self.host.invoke_scoped_typed::<catalog::ListDiscussionsV2>(DiscussionListQuery {
                parent: input.parent, allowed_parent_kinds: kinds,
                status: input.status, limit: input.limit.unwrap_or(50), cursor: input.cursor,
            }).await.map_err(operation_error)?;
            Ok(Stamped {
                result: DiscussionListPage { entries: response.result.entries, next_cursor: response.result.next_cursor },
                stamp: response.stamp, freshness_error: response.freshness_error,
            })
        })
    }

    fn conversation(&self, input: ConversationInput) -> PortFuture<'_, Stamped<DiscussionConversation>> {
        Box::pin(async move {
            let kinds = self.permitted_parent_kinds::<catalog::ReviewDiscussionV2>();
            if kinds.is_empty() { return Err(DiscussionError::AccessDenied); }
            let response = self.host.invoke_scoped_typed::<catalog::GetDiscussionConversationV2>(DiscussionConversationQuery {
                discussion_id: input.discussion_id, allowed_parent_kinds: kinds,
                limit: input.limit.unwrap_or(50), cursor: input.cursor,
            }).await.map_err(operation_error)?;
            Ok(Stamped {
                result: DiscussionConversation {
                    head: response.result.head,
                    messages: provenance_core::threads::DiscussionMessagesPage {
                        entries: response.result.messages.entries,
                        next_cursor: response.result.messages.next_cursor,
                    },
                },
                stamp: response.stamp, freshness_error: response.freshness_error,
            })
        })
    }

    fn start(&self, input: StartInput) -> PortFuture<'_, DiscussionEntry> {
        Box::pin(async move {
            let kinds = self.permitted_write_parent_kinds(DiscussionAction::Discuss);
            if !kinds.contains(&input.parent.node_type) { return Err(DiscussionError::AccessDenied); }
            self.host.invoke_scope_typed::<catalog::WriteTargetDiscussionV2>(TargetDiscussionWrite {
                scope_id: self.scope()?, request_id: input.request_id,
                actor: input.actor, declared_by: input.declared_by,
                allowed_parent_kinds: kinds,
                action: TargetDiscussionAction::Start { parent: input.parent, role: input.role, body: input.body },
            }).await.map_err(operation_error)
        })
    }

    fn reply(&self, input: ReplyInput) -> PortFuture<'_, DiscussionEntry> {
        Box::pin(async move {
            let kinds = self.permitted_write_parent_kinds(DiscussionAction::Reply);
            if kinds.is_empty() {
                return Err(DiscussionError::AccessDenied);
            }
            self.host.invoke_scope_typed::<catalog::WriteTargetDiscussionV2>(TargetDiscussionWrite {
                scope_id: self.scope()?, request_id: input.request_id,
                actor: input.actor, declared_by: input.declared_by,
                allowed_parent_kinds: kinds,
                action: TargetDiscussionAction::Reply {
                    discussion_id: input.discussion_id, expected_version: input.expected_version,
                    role: input.role, body: input.body,
                },
            }).await.map_err(operation_error)
        })
    }
}

fn operation_error<E: std::error::Error + serde::Serialize>(error: OperationError<E>) -> DiscussionError {
    let detail = serde_json::to_value(&error).unwrap_or_else(|_| serde_json::json!({"kind":"internal"}));
    DiscussionError::Operation { message: error.to_string(), detail }
}

pub(super) fn is_available(host: &crate::StatementHost, action: DiscussionAction) -> bool {
    let port = HostDiscussionPort::new(host.clone());
    match action {
        DiscussionAction::Discussions => host.advertises(catalog::ListDiscussionsV2::NAME)
            && !port.permitted_parent_kinds::<catalog::ReviewDiscussionsV2>().is_empty(),
        DiscussionAction::Discussion => host.advertises(catalog::GetDiscussionConversationV2::NAME)
            && !port.permitted_parent_kinds::<catalog::ReviewDiscussionV2>().is_empty(),
        DiscussionAction::Discuss | DiscussionAction::Reply =>
            host.advertises(catalog::WriteTargetDiscussionV2::NAME)
            && !port.permitted_write_parent_kinds(action).is_empty(),
    }
}
