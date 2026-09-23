use super::{
    ContextKind, ExecutionNeed, ExecutionNeeds, Operation, OperationFuture, PreparedContext,
};
use crate::{
    layout::ProvenanceLayout,
    review,
    state_store::StateStore,
    write_error::{SourceFailure, WriteError, WriteFailure},
};
use provenance_core::threads::DiscussionEntry;

pub struct WriteTargetDiscussionV2;

impl Operation for WriteTargetDiscussionV2 {
    type Request = review::TargetDiscussionWrite;
    type Success = DiscussionEntry;
    type Failure = WriteError;
    const NAME: &'static str = "write-target-discussion-v2";
    const MUTATES: bool = true;
    const CONTEXT: ContextKind = ContextKind::Scope;
    const FAILURE_STATUSES: &'static [u16] = &[409];

    fn needs(_: &Self::Request) -> ExecutionNeeds {
        &[ExecutionNeed::GraphStorage]
    }

    fn failure_status(error: &WriteError) -> u16 {
        error.status()
    }

    fn run(
        context: PreparedContext,
        request: Self::Request,
    ) -> OperationFuture<Self::Success, Self::Failure> {
        Box::pin(async move {
            let context = context.scope()?;
            if request.scope_id != context.scope {
                return Err(SourceFailure::wrap(
                    WriteFailure::ScopeMismatch,
                    anyhow::anyhow!("request scope does not match selected scope"),
                )
                .into());
            }
            StateStore::new(ProvenanceLayout::new(context.root))
                .write_target_discussion(request)
                .map_err(Into::into)
        })
    }
}
