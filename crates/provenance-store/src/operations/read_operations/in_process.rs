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
use std::sync::Arc;

/// Runs each read operation on one opened repository, bound under one
/// target name.
///
/// Each call is checked and authorized as [`LocalRepository`] checks it for
/// the router, so a call that names another target refuses with
/// `unknown_target`. Then the repository is prepared and the operation runs.
#[derive(Clone)]
pub struct InProcessReadOperations {
    binding: Arc<LocalRepository>,
}

impl InProcessReadOperations {
    /// The read operations of `repository`, under the target name `alias`.
    ///
    /// A port that `repository` holds is not used: the operations run here.
    pub fn new(repository: Repository, alias: &str) -> Result<Self, OperationFailure> {
        Ok(Self {
            binding: Arc::new(LocalRepository::bind(repository, alias)?),
        })
    }
}

impl std::fmt::Debug for InProcessReadOperations {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InProcessReadOperations")
            .field("target", &self.binding.repository())
            .finish_non_exhaustive()
    }
}

impl ReadOperations for InProcessReadOperations {
    fn run(&self, operation: &'static str, request: Value) -> ReadAnswer {
        let Some(entry) = entries().into_iter().find(|entry| entry.name == operation) else {
            return Box::pin(async {
                Err(ErasedFailure::new(None, OperationFailure::UnknownOperation))
            });
        };
        (entry.run)(Arc::clone(&self.binding), request)
    }
}

struct Entry {
    name: &'static str,
    run: fn(Arc<LocalRepository>, Value) -> ReadAnswer,
}

const fn entry<O: Operation>() -> Entry {
    Entry {
        name: O::NAME,
        run: run::<O>,
    }
}

/// The operations of [`super::READ_OPERATIONS`], in its order.
const fn entries() -> [Entry; 10] {
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

/// The context and the request of `call`, after the outside-call check.
fn check<O: Operation>(call: Value) -> Result<(RequestedContext, O::Request), OperationFailure> {
    let call: Call<O::Request> = decode(call)?;
    let selected = selected(O::CONTEXT, call.context)?;
    O::validate_external(&call.request)?;
    Ok((selected, call.request))
}

/// Checks the call, authorizes it, prepares the repository, then runs the
/// operation.
///
/// The order is the order of the router's own invocation. The failure is
/// the dispatch's own failure type, which the port gives as it is.
fn run<O: Operation>(binding: Arc<LocalRepository>, call: Value) -> ReadAnswer {
    Box::pin(async move {
        let refuse = |failure| ErasedFailure::new(Some(O::NAME), failure);
        let (selected, request) = check::<O>(call).map_err(refuse)?;
        binding.authorize(O::NAME, &selected).map_err(refuse)?;
        let context = binding
            .prepare(O::NAME, selected, O::needs(&request))
            .map_err(refuse)?;
        let success = invoke_typed::<O>(context, request)
            .await
            .map_err(frame_failure::<O>)?;
        serde_json::to_value(success).map_err(|_| refuse(OperationFailure::Internal))
    })
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
