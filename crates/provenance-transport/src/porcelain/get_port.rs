use provenance_core::protocol::{
    Direction, ImpactQuery, QueryResponse, ResolveRecordQuery, ResponseMeta, TraceQuery,
};
use provenance_core::threads::{
    DiscussionConversationQuery, DiscussionConversationResult, DiscussionListQuery,
    DiscussionResultPage, DiscussionStatusFilter,
};
use provenance_core::{NodeType, StableId, SDK_PROTOCOL_VERSION};
use provenance_porcelain::get::{
    Bounds, GetPort, Impact, PortFuture, ReadError, RecordResolution, Review, Traversal,
    TraversalRequest, View,
};
use provenance_store::operations::catalog::{self, Operation as _};

/// Typed graph operations adapted to the Porcelain read port.
#[derive(Clone)]
pub struct HostGetPort {
    host: crate::StatementHost,
}

struct ReviewDiscussions {
    page: DiscussionResultPage<DiscussionConversationResult>,
    follow_up_commands: Vec<String>,
    stamp: provenance_core::protocol::Stamp,
    freshness_error: Option<String>,
}

impl HostGetPort {
    pub const fn new(host: crate::StatementHost) -> Self {
        Self { host }
    }

    fn permitted_node_types(&self) -> Vec<NodeType> {
        permitted_node_types(&self.host)
    }
}

pub(super) fn permitted_node_types(host: &crate::StatementHost) -> Vec<NodeType> {
    catalog::definitions()
        .iter()
        .filter(|definition| host.advertises(definition.name))
        .filter_map(|definition| {
            definition
                .registration
                .queries
                .iter()
                .find(|query| query.name == catalog::Trace::NAME)
                .and_then(|query| query.request.node_type)
                .and_then(|node_type| NodeType::parse(node_type).ok())
        })
        .fold(Vec::new(), |mut permitted, node_type| {
            if !permitted.contains(&node_type) {
                permitted.push(node_type);
            }
            permitted
        })
}

impl GetPort for HostGetPort {
    fn resolve<'a>(&'a self, id: &'a str) -> PortFuture<'a, RecordResolution> {
        Box::pin(async move {
            let response = self
                .host
                .invoke_scoped_typed::<catalog::ResolveRecord>(ResolveRecordQuery {
                    protocol_version: Some(SDK_PROTOCOL_VERSION),
                    id: id.to_owned(),
                    allowed_node_types: self.permitted_node_types(),
                })
                .await
                .map_err(|error| operation_error(&error))?;
            let (result, metadata) = response_parts(response);
            Ok(RecordResolution {
                result: result.resolution,
                metadata: Some(metadata),
            })
        })
    }

    fn traverse(&self, request: TraversalRequest) -> PortFuture<'_, Traversal> {
        Box::pin(async move {
            let response = self
                .host
                .invoke_scoped_typed::<catalog::Trace>(TraceQuery {
                    protocol_version: Some(SDK_PROTOCOL_VERSION),
                    id: request.target,
                    node_type: Some(request.kind),
                    direction: if request.view == View::Children {
                        Direction::In
                    } else {
                        Direction::Out
                    },
                    relations: Vec::new(),
                    max_depth: request.max_depth,
                    limit: request.limit,
                })
                .await
                .map_err(|error| operation_error(&error))?;
            let (mut result, mut metadata) = response_parts(response);
            let permitted = self.permitted_node_types();
            result
                .nodes
                .retain(|record| permitted.contains(&record.node.node_type()));
            metadata.limit = Some(result.limit);
            metadata.has_more = Some(result.has_more);
            Ok(Traversal {
                records: result.nodes,
                bounds: Bounds {
                    limit: result.limit,
                    max_depth: Some(result.max_depth),
                    has_more: result.has_more,
                    continuation: None,
                    truncated: result.has_more,
                },
                response_metadata: Some(metadata),
            })
        })
    }

    fn impact<'a>(
        &'a self,
        record: &'a provenance_core::protocol::GraphNode,
        limit: usize,
    ) -> PortFuture<'a, Impact> {
        Box::pin(async move {
            let permitted = self.permitted_node_types();
            if !NodeType::ALL
                .iter()
                .all(|node_type| permitted.contains(node_type))
            {
                return Err(ReadError::InvalidOptions);
            }
            let response = self
                .host
                .invoke_scoped_typed::<catalog::Impact>(ImpactQuery {
                    protocol_version: Some(SDK_PROTOCOL_VERSION),
                    id: record.id().as_str().to_owned(),
                    node_type: Some(record.node_type()),
                    limit,
                })
                .await
                .map_err(|error| operation_error(&error))?;
            let (result, mut metadata) = response_parts(response);
            metadata.limit = Some(result.limit);
            metadata.has_more = Some(result.has_more);
            let truncated = result.has_more || result.scan_cut;
            Ok(Impact {
                bounds: Bounds {
                    limit: result.limit,
                    max_depth: None,
                    has_more: result.has_more,
                    continuation: None,
                    truncated,
                },
                detail: result,
                response_metadata: Some(metadata),
            })
        })
    }

    /// Returns the current decision, edit guard, and feedback for each record kind.
    #[provenance_macros::rule("rule_porcelain_review_returns_current_state")]
    fn review<'a>(
        &'a self,
        record: &'a provenance_core::protocol::GraphNode,
        limit: usize,
    ) -> PortFuture<'a, Review> {
        Box::pin(async move {
            let kind = record.node_type();
            let id = record.id().clone();
            let resource = self
                .host
                .invoke_scoped_typed::<catalog::GetReviewedResource>(
                    catalog::ReviewedResourceRequest {
                        record_kind: kind,
                        id: id.clone(),
                    },
                )
                .await
                .map_err(|error| operation_error(&error))?;
            let edit = resource.result.edit;
            let decision = resource.result.decision;
            let loaded = if discussion_parent_is_supported(&self.host, kind) {
                load_review_discussions(&self.host, kind, &id, limit).await?
            } else {
                ReviewDiscussions {
                    page: DiscussionResultPage {
                        entries: Vec::new(),
                        limit,
                        has_more: false,
                        next_cursor: None,
                    },
                    follow_up_commands: Vec::new(),
                    stamp: resource.stamp,
                    freshness_error: resource.freshness_error,
                }
            };
            let discussions = loaded.page;
            let inner_has_more = discussions
                .entries
                .iter()
                .any(|conversation| conversation.messages.has_more);
            let truncated = discussions.has_more || inner_has_more;
            Ok(Review {
                update_precondition: format!("--if-match {}", edit.etag),
                edit,
                decision,
                bounds: Bounds {
                    limit: discussions.limit,
                    max_depth: None,
                    has_more: truncated,
                    continuation: discussions.next_cursor.clone(),
                    truncated,
                },
                discussions,
                follow_up_commands: loaded.follow_up_commands,
                response_metadata: Some(provenance_core::protocol::ResponseMeta {
                    stamp: Some(loaded.stamp),
                    freshness_error: loaded.freshness_error,
                    ..provenance_core::protocol::ResponseMeta::default()
                }),
            })
        })
    }
}

async fn load_review_discussions(
    host: &crate::StatementHost,
    kind: NodeType,
    id: &StableId,
    limit: usize,
) -> Result<ReviewDiscussions, ReadError> {
    let listed = host
        .invoke_scoped_typed::<catalog::ListDiscussions>(DiscussionListQuery {
            parent: Some(provenance_core::ThreadParent {
                node_type: kind,
                node_id: id.clone(),
            }),
            allowed_parent_kinds: vec![kind],
            status: DiscussionStatusFilter::All,
            limit,
            cursor: None,
        })
        .await
        .map_err(|error| operation_error(&error))?;
    let mut follow_up_commands = Vec::new();
    if let Some(cursor) = &listed.result.next_cursor {
        follow_up_commands.push(format!(
            "provenance {} discussions --limit {limit} --cursor {cursor}",
            id.as_str()
        ));
    }
    let mut conversations = Vec::with_capacity(listed.result.entries.len());
    for summary in &listed.result.entries {
        let conversation = host
            .invoke_scoped_typed::<catalog::GetDiscussionConversation>(
                DiscussionConversationQuery {
                    discussion_id: summary.discussion_id.clone(),
                    allowed_parent_kinds: vec![kind],
                    limit,
                    cursor: None,
                },
            )
            .await
            .map_err(|error| operation_error(&error))?;
        if let Some(cursor) = &conversation.result.messages.next_cursor {
            follow_up_commands.push(format!(
                "provenance discussions {} get --limit {limit} --cursor {cursor}",
                summary.discussion_id.as_str()
            ));
        }
        conversations.push(conversation.result);
    }
    Ok(ReviewDiscussions {
        page: DiscussionResultPage {
            entries: conversations,
            limit,
            has_more: listed.result.has_more,
            next_cursor: listed.result.next_cursor,
        },
        follow_up_commands,
        stamp: listed.stamp,
        freshness_error: listed.freshness_error,
    })
}

fn discussion_parent_is_supported(host: &crate::StatementHost, kind: NodeType) -> bool {
    catalog::definitions().iter().any(|definition| {
        host.advertises(definition.name)
            && definition.registration.handler.operation == catalog::ReviewDiscussions::NAME
            && definition
                .registration
                .request
                .parent
                .as_ref()
                .is_some_and(|parent| parent.kind == kind.as_str())
    })
}

fn response_parts<R>(response: QueryResponse<R>) -> (R, ResponseMeta) {
    (
        response.result,
        ResponseMeta {
            stamp: Some(response.stamp),
            freshness_error: response.freshness_error,
            freshness_cause: response.freshness_cause,
            ..ResponseMeta::default()
        },
    )
}

fn operation_error<E: std::error::Error>(
    error: &provenance_core::protocol::failure::OperationError<E>,
) -> ReadError {
    ReadError::Operation(error.to_string())
}

pub(super) fn is_available(host: &crate::StatementHost) -> bool {
    host.advertises(catalog::ResolveRecord::NAME)
        && catalog::definitions().iter().any(|definition| {
            host.advertises(definition.name)
                && definition
                    .registration
                    .queries
                    .iter()
                    .any(|query| query.name == catalog::Trace::NAME)
        })
}

pub(super) fn resolver_permits(host: &crate::StatementHost, kind: NodeType) -> bool {
    host.advertises(catalog::ResolveRecord::NAME) && permitted_node_types(host).contains(&kind)
}

#[cfg(test)]
mod tests {
    use provenance_core::protocol::{
        RecordResolution as CoreRecordResolution, ResolveRecordResult,
    };

    #[test]
    fn a_core_resolution_stays_typed_at_the_adapter_boundary() {
        let result = ResolveRecordResult {
            resolution: CoreRecordResolution::Missing,
        };
        assert!(matches!(result.resolution, CoreRecordResolution::Missing));
    }
}
