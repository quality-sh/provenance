use provenance_core::{
    protocol::{failure::OperationError, Stamped},
    threads::{
        DiscussionConversationQuery, DiscussionConversationResult, DiscussionEntry,
        DiscussionListQuery,
    },
    NodeType, ScopeId,
};
use provenance_porcelain::{action::Action, discussion::{
    ConversationInput, DiscussionError, DiscussionPort, ListAnswer, ListInput,
    PortFuture, ReplyInput, StartInput,
}};
use provenance_store::{
    operations::catalog::{self, DiscussionWriteKind, Operation as _},
    review::{TargetDiscussionAction, TargetDiscussionWrite},
};

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
        catalog::definitions()
            .iter()
            .filter(|definition| definition.registration.handler.operation == O::NAME)
            .filter(|definition| self.host.advertises(definition.name))
            .filter_map(|definition| definition.registration.request.parent.as_ref())
            .filter_map(|parent| NodeType::parse(parent.kind).ok())
            .collect()
    }

    fn permitted_write_parent_kinds(&self, action: Action) -> Vec<NodeType> {
        let kind = match action {
            Action::Discuss => DiscussionWriteKind::Start,
            Action::Reply => DiscussionWriteKind::Reply,
            _ => return Vec::new(),
        };
        write_parent_kinds(catalog::definitions(), kind, |name| {
            self.host.advertises(name)
        })
    }

    fn scope(&self) -> Result<ScopeId, DiscussionError> {
        self.host
            .bound_identity()
            .map(|(_, scope)| scope)
            .ok_or(DiscussionError::AccessDenied)
            .and_then(|scope| ScopeId::new(scope).map_err(|_| DiscussionError::AccessDenied))
    }
}

fn write_parent_kinds(
    definitions: &[catalog::Definition],
    kind: DiscussionWriteKind,
    advertises: impl Fn(&str) -> bool,
) -> Vec<NodeType> {
    definitions
        .iter()
        .filter(|definition| {
            definition.registration.handler.operation == catalog::WriteDiscussionV2::NAME
                && definition.registration.request.adapter.discussion_write == Some(kind)
                && advertises(definition.name)
        })
        .filter_map(|definition| definition.registration.request.parent.as_ref())
        .filter_map(|parent| NodeType::parse(parent.kind).ok())
        .collect()
}

impl DiscussionPort for HostDiscussionPort {
    fn list(&self, input: ListInput) -> PortFuture<'_, ListAnswer> {
        Box::pin(async move {
            let kinds = self.permitted_parent_kinds::<catalog::ReviewDiscussionsV2>();
            if kinds.is_empty() {
                return Err(DiscussionError::AccessDenied);
            }
            if input
                .parent
                .as_ref()
                .is_some_and(|parent| !kinds.contains(&parent.node_type))
            {
                return Err(DiscussionError::AccessDenied);
            }
            let response = self
                .host
                .invoke_scoped_typed::<catalog::ListDiscussionsV2>(DiscussionListQuery {
                    parent: input.parent,
                    allowed_parent_kinds: kinds,
                    status: input.status,
                    limit: input.limit.unwrap_or(50),
                    cursor: input.cursor,
                })
                .await
                .map_err(|error| operation_error(&error))?;
            let page = Stamped {
                result: response.result,
                stamp: response.stamp,
                freshness_error: response.freshness_error,
            };
            Ok(ListAnswer {
                scope_id: self.scope()?,
                page,
            })
        })
    }

    fn conversation(
        &self,
        input: ConversationInput,
    ) -> PortFuture<'_, Stamped<DiscussionConversationResult>> {
        Box::pin(async move {
            let kinds = self.permitted_parent_kinds::<catalog::ReviewDiscussionV2>();
            if kinds.is_empty() {
                return Err(DiscussionError::AccessDenied);
            }
            let response = self
                .host
                .invoke_scoped_typed::<catalog::GetDiscussionConversationV2>(
                    DiscussionConversationQuery {
                        discussion_id: input.discussion_id,
                        allowed_parent_kinds: kinds,
                        limit: input.limit.unwrap_or(50),
                        cursor: input.cursor,
                    },
                )
                .await
                .map_err(|error| operation_error(&error))?;
            Ok(Stamped {
                result: response.result,
                stamp: response.stamp,
                freshness_error: response.freshness_error,
            })
        })
    }

    fn start(&self, input: StartInput) -> PortFuture<'_, DiscussionEntry> {
        Box::pin(async move {
            let kinds = self.permitted_write_parent_kinds(Action::Discuss);
            if !kinds.contains(&input.parent.node_type) {
                return Err(DiscussionError::AccessDenied);
            }
            self.host
                .invoke_scope_typed::<catalog::WriteTargetDiscussionV2>(TargetDiscussionWrite {
                    scope_id: self.scope()?,
                    request_id: input.request_id,
                    actor: input.actor,
                    declared_by: input.declared_by,
                    allowed_parent_kinds: kinds,
                    action: TargetDiscussionAction::Start {
                        parent: input.parent,
                        role: input.role,
                        body: input.body,
                    },
                })
                .await
                .map_err(|error| operation_error(&error))
        })
    }

    fn reply(&self, input: ReplyInput) -> PortFuture<'_, DiscussionEntry> {
        Box::pin(async move {
            let kinds = self.permitted_write_parent_kinds(Action::Reply);
            if kinds.is_empty() {
                return Err(DiscussionError::AccessDenied);
            }
            self.host
                .invoke_scope_typed::<catalog::WriteTargetDiscussionV2>(TargetDiscussionWrite {
                    scope_id: self.scope()?,
                    request_id: input.request_id,
                    actor: input.actor,
                    declared_by: input.declared_by,
                    allowed_parent_kinds: kinds,
                    action: TargetDiscussionAction::Reply {
                        discussion_id: input.discussion_id,
                        expected_version: input.expected_version,
                        role: input.role,
                        body: input.body,
                    },
                })
                .await
                .map_err(|error| operation_error(&error))
        })
    }
}

fn operation_error<E: std::error::Error + serde::Serialize>(
    error: &OperationError<E>,
) -> DiscussionError {
    let detail =
        serde_json::to_value(error).unwrap_or_else(|_| serde_json::json!({"kind":"internal"}));
    DiscussionError::Operation {
        message: error.to_string(),
        detail,
    }
}

pub(super) fn is_available(host: &crate::StatementHost, action: Action) -> bool {
    let port = HostDiscussionPort::new(host.clone());
    match action {
        Action::Discussions => {
            host.advertises(catalog::ListDiscussionsV2::NAME)
                && !port
                    .permitted_parent_kinds::<catalog::ReviewDiscussionsV2>()
                    .is_empty()
        }
        Action::Discussion => {
            host.advertises(catalog::GetDiscussionConversationV2::NAME)
                && !port
                    .permitted_parent_kinds::<catalog::ReviewDiscussionV2>()
                    .is_empty()
        }
        Action::Discuss | Action::Reply => {
            host.advertises(catalog::WriteTargetDiscussionV2::NAME)
                && !port.permitted_write_parent_kinds(action).is_empty()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_grants_follow_the_registered_action_after_a_route_rename() {
        let mut definitions = catalog::definitions().to_vec();
        let start = definitions
            .iter_mut()
            .find(|definition| definition.name == "requirements-create-discussion")
            .unwrap();
        start.name = "renamed-requirement-start";
        let kinds = write_parent_kinds(&definitions, DiscussionWriteKind::Start, |name| {
            name == "renamed-requirement-start"
        });
        assert_eq!(kinds, vec![NodeType::Requirement]);
        let reply = write_parent_kinds(&definitions, DiscussionWriteKind::Reply, |name| {
            name == "requirements-create-discussion-message"
        });
        assert_eq!(reply, vec![NodeType::Requirement]);
        let denied = write_parent_kinds(&definitions, DiscussionWriteKind::Start, |name| {
            name == "requirements-create-discussion-message"
        });
        assert!(denied.is_empty());
    }
}
