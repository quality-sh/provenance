//! Typed invocation and failure framing. Dispatch by name lives in `provenance-contract`.

use super::{CallFailure as _, Operation, PreparedContext};
#[cfg(any(test, feature = "test-fixture"))]
use super::{Invoke, OperationFuture};
use provenance_core::protocol::failure::ErasedFailure as FailureEnvelope;

/// Native callers cross the same preparation and handler seam without JSON conversion.
pub async fn invoke_typed<O: Operation>(
    context: PreparedContext,
    request: O::Request,
) -> Result<O::Success, O::Failure> {
    context.validate_kind(O::CONTEXT)?;
    let context = context.prepare(O::needs(&request))?;
    O::run(context, request).await
}

/// Authorize one typed call before it crosses the operation boundary: the identity check
/// first, then repository preparation.
pub async fn invoke_authorized_typed<O: Operation>(
    resolver: std::sync::Arc<dyn super::ContextResolver>,
    selected: super::RequestedContext,
    request: O::Request,
) -> Result<O::Success, O::Failure> {
    O::validate_external(&request)?;
    resolver.authorize(O::NAME, &selected)?;
    let context = resolver.prepare(O::NAME, selected, O::needs(&request))?;
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
    ) -> OperationFuture<O::Success, O::Failure> {
        Box::pin(async move {
            let Some(selected) = selected else {
                O::validate_external(&request)?;
                return invoke_typed::<O>(PreparedContext::data_free(), request).await;
            };
            invoke_authorized_typed::<O>(self.0, selected, request).await
        })
    }
}

/// Frames one typed refusal as the failure envelope that the operation declares.
pub fn frame_failure<O: Operation>(failure: O::Failure) -> FailureEnvelope {
    let status = failure.failure_status();
    let cause = failure.log_cause();
    FailureEnvelope::declared(O::NAME, failure, status).with_cause(cause)
}
