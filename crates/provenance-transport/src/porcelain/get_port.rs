use provenance_core::protocol::{
    Direction, ImpactQuery, QueryResponse, ResolveRecordQuery, ResponseMeta, TraceQuery,
};
use provenance_core::{NodeType, SDK_PROTOCOL_VERSION};
use provenance_porcelain::get::{
    Bounds, GetPort, Impact, PortFuture, ReadError, RecordResolution, Traversal, TraversalRequest,
    View,
};
use provenance_store::operations::catalog::{self, Operation as _};

/// Typed graph operations adapted to the Porcelain read port.
#[derive(Clone)]
pub struct HostGetPort {
    host: crate::StatementHost,
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

pub(super) async fn resolve(
    host: &crate::StatementHost,
    id: &str,
) -> Result<provenance_core::protocol::RecordResolution, String> {
    let response = host
        .invoke_scoped_typed::<catalog::ResolveRecord>(ResolveRecordQuery {
            protocol_version: Some(SDK_PROTOCOL_VERSION),
            id: id.to_owned(),
            allowed_node_types: permitted_node_types(host),
        })
        .await
        .map_err(|error| error.to_string())?;
    Ok(response.result.resolution)
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

pub(super) fn is_resolver_available(host: &crate::StatementHost) -> bool {
    host.advertises(catalog::ResolveRecord::NAME) && !permitted_node_types(host).is_empty()
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
