//! Existing native proposal-lifecycle operations keep their native handlers.
//! The three reads return the complete native record arrays; the three
//! creations call the existing writers behind their lifecycle locks. Reads
//! use `StateStore::list_proposal_cards`, the validated native effective-state
//! projection, and never the actor-override projection or the raw definition
//! list.
use super::{scoped_list::scoped_list, shapes::scoped_write_operation, ExecutionNeed};
use crate::state_store::{CreateAssertionInput, CreateDispositionInput, CreateProposalCardInput};

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
        scoped_write_operation!(
            pub $name, $wire, $input, $output, &[], &[ExecutionNeed::GraphStorage],
            scope = scope_id, |store, _scope, request| store.$method(request)
        );
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
