use provenance_core::protocol::{failure::OperationError, SearchQuery};
use provenance_porcelain::search::{PortFuture, SearchError, SearchPort};
use provenance_store::operations::catalog::{self, Operation as _};

/// The canonical typed search operation with host kind grants applied.
#[derive(Clone)]
pub struct HostSearchPort {
    host: crate::StatementHost,
}

impl HostSearchPort {
    pub const fn new(host: crate::StatementHost) -> Self {
        Self { host }
    }
}

impl SearchPort for HostSearchPort {
    fn search(&self, mut request: SearchQuery) -> PortFuture<'_> {
        Box::pin(async move {
            let permitted = super::get_port::permitted_node_types(&self.host);
            if permitted.is_empty() {
                return Err(SearchError::AccessDenied);
            }
            if request.node_types.is_empty() {
                request.node_types = permitted;
            } else {
                request
                    .node_types
                    .retain(|kind| permitted.contains(kind));
                if request.node_types.is_empty() {
                    return Err(SearchError::AccessDenied);
                }
            }
            self.host
                .invoke_scoped_typed::<catalog::Search>(request)
                .await
                .map_err(|error| operation_error(&error))
        })
    }
}

fn operation_error(
    error: &OperationError<<catalog::Search as catalog::Operation>::Failure>,
) -> SearchError {
    let message = error.to_string();
    let detail = serde_json::to_value(error)
        .unwrap_or_else(|_| serde_json::json!({"kind":"internal"}));
    SearchError::Operation { message, detail }
}

pub(super) fn is_available(host: &crate::StatementHost) -> bool {
    host.advertises(catalog::Search::NAME)
        && !super::get_port::permitted_node_types(host).is_empty()
}
