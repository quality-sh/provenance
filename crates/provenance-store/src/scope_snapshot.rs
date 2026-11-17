//! Every record family of one scope, read in one publication section.

use crate::state_store::StateStore;
use provenance_core::{
    AssertionRecord, Contribution, DispositionRecord, ImplementationBinding, Message, ProposalCard,
    Question, Requirement, Resolution, Rule, ScopeId, Source, SynthesisPacket, Thread, Topic,
    VerificationBinding,
};

/// Every family of one scope, read under one publication section. Takes the
/// scope because repository-wide callers read each manifest scope in turn.
pub fn snapshot(store: &StateStore, scope: &ScopeId) -> anyhow::Result<ScopeSnapshot> {
    store.with_repository_publication(|held| {
        Ok(ScopeSnapshot {
            sources: held.list_sources(scope)?,
            requirements: held.list_requirements(scope)?,
            topics: held.list_topics(scope)?,
            questions: held.list_questions(scope)?,
            resolutions: held.list_resolutions(scope)?,
            rules: held.list_rules(scope)?,
            verification_bindings: held.list_verification_bindings(scope)?,
            implementation_bindings: held.list_implementation_bindings(scope)?,
            threads: held.list_threads(scope)?,
            messages: held.list_messages(scope)?,
            contributions: held.list_contributions(scope)?,
            synthesis_packets: held.list_synthesis_packets(scope)?,
            proposal_cards: held.list_proposal_definitions(scope)?,
            assertion_records: held.list_assertion_records(scope)?,
            dispositions: held.list_dispositions(scope)?,
        })
    })
}

#[derive(Debug)]
pub struct ScopeSnapshot {
    pub sources: Vec<Source>,
    pub requirements: Vec<Requirement>,
    pub topics: Vec<Topic>,
    pub questions: Vec<Question>,
    pub resolutions: Vec<Resolution>,
    pub rules: Vec<Rule>,
    pub verification_bindings: Vec<VerificationBinding>,
    pub implementation_bindings: Vec<ImplementationBinding>,
    pub threads: Vec<Thread>,
    pub messages: Vec<Message>,
    pub contributions: Vec<Contribution>,
    pub synthesis_packets: Vec<SynthesisPacket>,
    pub proposal_cards: Vec<ProposalCard>,
    pub assertion_records: Vec<AssertionRecord>,
    pub dispositions: Vec<DispositionRecord>,
}

impl ScopeSnapshot {
    /// The graph families of the snapshot, which the reference check reads.
    pub(crate) fn graph_records(&self) -> GraphRecords<'_> {
        GraphRecords {
            sources: &self.sources,
            requirements: &self.requirements,
            topics: &self.topics,
            questions: &self.questions,
            resolutions: &self.resolutions,
            rules: &self.rules,
            verification_bindings: &self.verification_bindings,
            implementation_bindings: &self.implementation_bindings,
        }
    }
}

/// Borrowed slices of the graph families of one [`ScopeSnapshot`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct GraphRecords<'a> {
    pub(crate) sources: &'a [Source],
    pub(crate) requirements: &'a [Requirement],
    pub(crate) topics: &'a [Topic],
    pub(crate) questions: &'a [Question],
    pub(crate) resolutions: &'a [Resolution],
    pub(crate) rules: &'a [Rule],
    pub(crate) verification_bindings: &'a [VerificationBinding],
    pub(crate) implementation_bindings: &'a [ImplementationBinding],
}

#[cfg(test)]
mod tests;
