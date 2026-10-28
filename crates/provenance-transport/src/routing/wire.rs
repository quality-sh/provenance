//! The routing adapter's host: the decoded wire context enters the one host invocation.
use crate::{Invocation, StatementHost};
use provenance_core::protocol::failure::OperationError;
use provenance_store::operations::catalog::{Invoke, Operation, OperationFuture, RequestedContext};

#[derive(Clone)]
pub struct WireCall {
    pub host: StatementHost,
    pub page_write: bool,
}

impl Invoke for WireCall {
    fn invoke<O: Operation>(
        self,
        selected: Option<RequestedContext>,
        request: O::Request,
    ) -> OperationFuture<O::Success, OperationError<O::Failure>> {
        Box::pin(async move {
            self.host
                .invoke::<O>(request, Invocation::wire(selected, self.page_write))
                .await
        })
    }
}
