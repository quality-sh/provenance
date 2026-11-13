//! Each engine sends its operations through `provenance_contract::invoke_with`
//! over the simulated storage, so the dispatch, the request decoding and the
//! failure framing run on every step.

use super::contract_laws::contract_laws;
use provenance_core::protocol::failure::{ErasedFailure, OperationFailure};
use provenance_core::SDK_PROTOCOL_VERSION;
use provenance_simulation::generate::{self, Effect, Task, SCOPE};
use provenance_simulation::Config;
use provenance_store::operations::catalog::{
    invoke_authorized_typed, ContextResolver, ExecutionNeeds, Invoke, Operation as Catalog,
    OperationFuture, PreparedContext, PreparedScope, RequestedContext,
};
use provenance_store::repository::Repository;
use serde_json::{json, Value};
use std::sync::Arc;

pub const CONTRACT: Config = Config {
    plant: None,
    only_law: None,
    apply: Some(apply_contract),
    layer_laws: Some(contract_laws),
};

/// Prepares every call for the one simulated repository of an engine.
struct Resolver(Repository);

impl ContextResolver for Resolver {
    fn authorize(&self, _: &'static str, _: &RequestedContext) -> Result<(), OperationFailure> {
        Ok(())
    }

    fn prepare(
        &self,
        _: &'static str,
        _: RequestedContext,
        _: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        Ok(PreparedContext::for_scope(PreparedScope {
            repository: self.0.clone(),
            requested_target: "simulated".into(),
        }))
    }
}

/// Runs each call on the engine's thread, as the host's blocking path does.
#[derive(Clone)]
struct Host(Arc<Resolver>);

impl Invoke for Host {
    fn invoke<O: Catalog>(
        self,
        selected: Option<RequestedContext>,
        request: O::Request,
    ) -> OperationFuture<O::Success, O::Failure> {
        Box::pin(async move {
            let selected = selected.ok_or(OperationFailure::Internal)?;
            invoke_authorized_typed::<O>(self.0, selected, request).await
        })
    }
}

/// Polls `future` on this thread. The catalog writes do not wait on a
/// runtime, so the first poll gives the answer.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    struct Unpark(std::thread::Thread);
    impl std::task::Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = std::task::Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut context = std::task::Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::park();
    }
}

/// The start of the error text of a contract refusal with `status`.
pub fn refusal_prefix(status: u16) -> String {
    format!("contract answer {status}:")
}

/// Sends one call by name through the contract dispatch.
pub fn invoke(
    repository: &Repository,
    operation: &str,
    request: &Value,
) -> Result<Value, Box<ErasedFailure>> {
    let call = json!({"context": {"repository": "simulated", "scope": SCOPE}, "request": request});
    let host = Host(Arc::new(Resolver(repository.clone())));
    block_on(provenance_contract::invoke_with(
        operation,
        SDK_PROTOCOL_VERSION,
        call,
        host,
    ))
    .map_err(Box::new)
}

/// Runs `task` through the contract; an operation with no contract call runs
/// on the store, as the engine layer does.
pub fn apply_contract(repository: &Repository, task: &Task) -> anyhow::Result<Effect> {
    let Some(call) = task.call(repository)? else {
        return generate::apply(repository, task);
    };
    match invoke(repository, call.operation, &call.request) {
        Ok(answer) => task.effect(&call, &answer),
        Err(failure) => anyhow::bail!(
            "{} {}",
            refusal_prefix(failure.status_code()),
            serde_json::to_string(&failure)?
        ),
    }
}
