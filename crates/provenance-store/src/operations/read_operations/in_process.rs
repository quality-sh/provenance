//! The read operations run in this process, on one opened repository.

use super::{ReadAnswer, ReadOperations};
use crate::operations::catalog::{
    frame_failure, invoke_typed, ContextKind, ContextResolver as _, GetReviewedResource, Operation,
    ReadDocument, RequestedContext, ReviewDiscussionMessages, ReviewDiscussions,
};
use crate::operations::family_operations::{
    PageQuestions, PageRequirements, PageResolutions, PageRules, PageSources, PageTopics,
};
use crate::operations::LocalRepository;
use crate::repository::Repository;
use provenance_core::protocol::failure::{ErasedFailure, InvalidInputReason, OperationFailure};
use provenance_core::protocol::repository::{RepositoryContext, RepositoryScope, RepositoryTarget};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

/// Runs each read operation on one opened repository. The repository is
/// prepared as [`LocalRepository`] prepares it for the router, under the
/// target name of the call.
#[derive(Debug, Clone)]
pub struct InProcessReadOperations {
    repository: Repository,
}

impl InProcessReadOperations {
    /// The read operations of `repository`. A port that `repository` holds
    /// is not used: the operations run here.
    pub const fn new(repository: Repository) -> Self {
        Self { repository }
    }
}

impl ReadOperations for InProcessReadOperations {
    fn run(&self, operation: &'static str, request: Value) -> ReadAnswer {
        let Some(entry) = entries().into_iter().find(|entry| entry.name == operation) else {
            return Box::pin(async {
                Err(ErasedFailure::new(None, OperationFailure::UnknownOperation))
            });
        };
        (entry.run)(self.repository.clone(), request)
    }
}

/// Decodes `call` and checks its request as the operation checks a call
/// from outside, with no repository read, and gives the context that the
/// call selects. A host checks a call here, and authorizes its context,
/// before it gives the call to a port.
pub fn checked_context(
    operation: &str,
    call: &Value,
) -> Result<RequestedContext, OperationFailure> {
    let entry = entries()
        .into_iter()
        .find(|entry| entry.name == operation)
        .ok_or(OperationFailure::UnknownOperation)?;
    (entry.check)(call.clone())
}

struct Entry {
    name: &'static str,
    check: fn(Value) -> Result<RequestedContext, OperationFailure>,
    run: fn(Repository, Value) -> ReadAnswer,
}

fn entry<O: Operation>() -> Entry {
    Entry {
        name: O::NAME,
        check: |call| check::<O>(call).map(|(selected, _)| selected),
        run: |repository, call| Box::pin(run::<O>(repository, call)),
    }
}

/// The operations of [`super::READ_OPERATIONS`], in its order.
fn entries() -> [Entry; 10] {
    [
        entry::<PageSources>(),
        entry::<PageRequirements>(),
        entry::<PageResolutions>(),
        entry::<PageRules>(),
        entry::<PageTopics>(),
        entry::<PageQuestions>(),
        entry::<GetReviewedResource>(),
        entry::<ReadDocument>(),
        entry::<ReviewDiscussions>(),
        entry::<ReviewDiscussionMessages>(),
    ]
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Call<R> {
    context: Value,
    request: R,
}

/// The context and the request of `call`, after the check that the
/// operation makes on a call from outside.
fn check<O: Operation>(call: Value) -> Result<(RequestedContext, O::Request), OperationFailure> {
    let call: Call<O::Request> = decode(call)?;
    let selected = selected(O::CONTEXT, call.context)?;
    O::validate_external(&call.request)?;
    Ok((selected, call.request))
}

/// Checks the call, prepares the repository, then runs the operation, in
/// the order of the router's own invocation.
async fn run<O: Operation>(repository: Repository, call: Value) -> Result<Value, ErasedFailure> {
    let refuse = |failure| ErasedFailure::new(Some(O::NAME), failure);
    let (selected, request) = check::<O>(call).map_err(refuse)?;
    let target = match &selected {
        RequestedContext::Repository(context) => &context.repository,
        RequestedContext::Scope(context) => &context.repository,
        RequestedContext::Scoped(context) => &context.repository,
    };
    let binding = LocalRepository::bind(repository, target).map_err(refuse)?;
    let context = binding
        .prepare(O::NAME, selected, O::needs(&request))
        .map_err(refuse)?;
    let success = invoke_typed::<O>(context, request)
        .await
        .map_err(frame_failure::<O>)?;
    serde_json::to_value(success).map_err(|_| refuse(OperationFailure::Internal))
}

fn selected(kind: ContextKind, context: Value) -> Result<RequestedContext, OperationFailure> {
    Ok(match kind {
        ContextKind::Repository => {
            RequestedContext::Repository(decode::<RepositoryTarget>(context)?)
        }
        ContextKind::Scope => RequestedContext::Scope(decode::<RepositoryScope>(context)?),
        ContextKind::Scoped => RequestedContext::Scoped(decode::<RepositoryContext>(context)?),
        ContextKind::DataFree => return Err(OperationFailure::UnavailableNeeds),
    })
}

fn decode<T: DeserializeOwned>(value: Value) -> Result<T, OperationFailure> {
    serde_json::from_value(value).map_err(|_| invalid())
}

const fn invalid() -> OperationFailure {
    OperationFailure::InvalidInput {
        field: None,
        reason: InvalidInputReason::InvalidValue,
    }
}

#[cfg(test)]
mod tests {
    /// Implementation aid: the published list names the operations that the
    /// adapter runs, in the same order.
    #[test]
    fn the_published_list_is_the_list_the_adapter_runs() {
        let names: Vec<_> = super::entries().iter().map(|entry| entry.name).collect();
        assert_eq!(names, super::super::READ_OPERATIONS);
    }
}
