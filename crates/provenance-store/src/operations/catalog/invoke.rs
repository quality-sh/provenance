//! Typed invocation and failure framing. Dispatch by name lives in `provenance-contract`.

#[cfg(any(test, feature = "test-fixture"))]
use super::{Invoke, OperationFuture};
use super::{Operation, PreparedContext};
use provenance_core::protocol::failure::{ErasedFailure as FailureEnvelope, OperationError};

/// Native callers cross the same preparation and handler seam without JSON conversion.
pub async fn invoke_typed<O: Operation>(
    context: PreparedContext,
    request: O::Request,
) -> Result<O::Success, OperationError<O::Failure>> {
    context
        .validate_kind(O::CONTEXT)
        .map_err(OperationError::Common)?;
    let context = context
        .prepare(O::needs(&request))
        .map_err(OperationError::Common)?;
    O::run(context, request)
        .await
        .map_err(OperationError::Handler)
}

/// Authorize one typed call before it crosses the operation boundary: the identity check
/// first, then repository preparation.
pub async fn invoke_authorized_typed<O: Operation>(
    resolver: std::sync::Arc<dyn super::ContextResolver>,
    selected: super::RequestedContext,
    request: O::Request,
) -> Result<O::Success, OperationError<O::Failure>> {
    O::validate_external(&request).map_err(OperationError::Common)?;
    resolver
        .authorize(O::NAME, &selected)
        .map_err(OperationError::Common)?;
    let context = resolver
        .prepare(O::NAME, selected, O::needs(&request))
        .map_err(OperationError::Common)?;
    invoke_typed::<O>(context, request).await
}

/// Tests only: runs on the caller's thread with no admission or tracking.
#[cfg(any(test, feature = "test-fixture"))]
#[derive(Clone)]
pub struct Direct(pub std::sync::Arc<dyn super::ContextResolver>);

#[cfg(any(test, feature = "test-fixture"))]
impl Invoke for Direct {
    fn invoke<O: Operation>(
        self,
        selected: Option<super::RequestedContext>,
        request: O::Request,
    ) -> OperationFuture<O::Success, OperationError<O::Failure>> {
        Box::pin(async move {
            let Some(selected) = selected else {
                O::validate_external(&request).map_err(OperationError::Common)?;
                return invoke_typed::<O>(PreparedContext::data_free(), request).await;
            };
            invoke_authorized_typed::<O>(self.0, selected, request).await
        })
    }
}

/// Frames one typed refusal as the failure envelope that the operation declares.
pub fn frame_failure<O: Operation>(failure: OperationError<O::Failure>) -> FailureEnvelope {
    match failure {
        OperationError::Common(error) => FailureEnvelope::new(Some(O::NAME), error),
        OperationError::Handler(error) => {
            let status = O::failure_status(&error);
            FailureEnvelope::declared(O::NAME, error, status)
        }
    }
}
