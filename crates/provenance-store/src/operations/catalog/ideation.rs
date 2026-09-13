//! Existing native proposal-lifecycle operations keep their native handlers.
//! The three reads return the complete native record arrays; the three
//! creations call the existing writers behind their lifecycle locks. Reads
//! use `StateStore::list_proposal_cards`, the validated native effective-state
//! projection, and never the actor-override projection or the raw definition
//! list.
use super::{
    scoped_list::scoped_list, ExecutionNeed, ExecutionNeeds, Operation, OperationFuture,
    PreparedContext,
};
use crate::{
    layout::ProvenanceLayout,
    state_store::{
        CreateAssertionInput, CreateDispositionInput, CreateProposalCardInput, StateStore,
    },
    write_error::{SourceFailure, WriteError, WriteFailure},
};

scoped_list!(
    ListProposals,
    "list-proposals",
    ProposalCard,
    list_proposal_cards
);
scoped_list!(
    ListDispositions,
    "list-dispositions",
    DispositionRecord,
    list_dispositions
);
scoped_list!(
    ListAssertions,
    "list-assertions",
    AssertionRecord,
    list_assertion_records
);

macro_rules! ideation_create {
    ($name:ident, $wire:literal, $input:ty, $output:ty, $method:ident) => {
        pub struct $name;
        impl Operation for $name {
            type Request = $input;
            type Success = $output;
            type Failure = WriteError;
            const NAME: &'static str = $wire;
            const MUTATES: bool = true;
            const CONTEXT: super::ContextKind = super::ContextKind::Scope;
            fn failure_status(error: &WriteError) -> u16 {
                error.status()
            }
            fn needs(_: &Self::Request) -> ExecutionNeeds {
                &[ExecutionNeed::GraphStorage]
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
                    Ok(StateStore::new(ProvenanceLayout::new(context.root)).$method(request)?)
                })
            }
        }
    };
}
ideation_create!(
    CreateProposal,
    "create-proposal",
    CreateProposalCardInput,
    provenance_core::ProposalCard,
    create_proposal_card
);
ideation_create!(
    CreateAssertion,
    "create-assertion",
    CreateAssertionInput,
    provenance_core::AssertionRecord,
    assert_proposal
);
ideation_create!(
    CreateDisposition,
    "create-disposition",
    CreateDispositionInput,
    provenance_core::DispositionRecord,
    create_disposition
);
