use provenance_porcelain::api::{ApiCatalog, ApiError, ApiInput, ApiPort, ApiPortFuture};
use serde_json::Value;

/// The canonical public-path routes adapted to the Porcelain api port.
#[derive(Clone)]
pub struct HostApiPort {
    host: crate::StatementHost,
}

impl HostApiPort {
    pub const fn new(host: crate::StatementHost) -> Self {
        Self { host }
    }
}

impl ApiPort for HostApiPort {
    fn invoke(&self, input: ApiInput) -> ApiPortFuture<'_, Value> {
        Box::pin(async move {
            let _ = input;
            todo!("api invocation")
        })
    }

    fn discover(&self) -> ApiCatalog {
        todo!("api discovery")
    }
}

/// Map one canonical transport refusal to the semantic api failure.
pub(crate) fn api_error(failure: &provenance_core::protocol::failure::ErasedFailure) -> ApiError {
    let _ = failure;
    todo!("api error mapping")
}

pub(super) fn is_available(host: &crate::StatementHost) -> bool {
    // Transitional RED stub: the api tool stays hidden until the port is real.
    let _ = host;
    false
}
