use provenance_core::{
    AssertionRecord, Boundary, Contribution, DispositionRecord, Domain, ImplementationBinding,
    Message, ProposalCard, Question, Requirement, Resolution, Rule, ScopeId, Source,
    SynthesisPacket, Thread, Topic, VerificationBinding,
};
use provenance_store::{repository::Repository, state_store::StateStore};

/// The live store of one opened repository.
#[derive(Debug)]
pub struct Store {
    repository: Repository,
    state: StateStore,
}

impl Store {
    pub fn new(repository: &Repository) -> Self {
        Self {
            repository: repository.clone(),
            state: repository.store(),
        }
    }

    pub const fn repository(&self) -> &Repository {
        &self.repository
    }

    /// Every family of the selected scope, read under one publication section.
    pub fn snapshot(&self) -> anyhow::Result<ScopeSnapshot> {
        snapshot(&self.state, self.repository.scope())
    }
}

/// Every family of one scope, read under one publication section. Takes the
/// scope because repository-wide callers read each manifest scope in turn.
pub fn snapshot(store: &StateStore, scope: &ScopeId) -> anyhow::Result<ScopeSnapshot> {
    store.with_repository_publication(|| {
        Ok(ScopeSnapshot {
            sources: store.list_sources(scope)?,
            domains: store.list_domains(scope)?,
            requirements: store.list_requirements(scope)?,
            boundaries: store.list_boundaries(scope)?,
            topics: store.list_topics(scope)?,
            questions: store.list_questions(scope)?,
            resolutions: store.list_resolutions(scope)?,
            rules: store.list_rules(scope)?,
            verification_bindings: store.list_verification_bindings(scope)?,
            implementation_bindings: store.list_implementation_bindings(scope)?,
            threads: store.list_threads(scope)?,
            messages: store.list_messages(scope)?,
            contributions: store.list_contributions(scope)?,
            synthesis_packets: store.list_synthesis_packets(scope)?,
            proposal_cards: store.list_proposal_definitions(scope)?,
            assertion_records: store.list_assertion_records(scope)?,
            dispositions: store.list_dispositions(scope)?,
        })
    })
}

impl std::ops::Deref for Store {
    type Target = StateStore;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

#[derive(Debug)]
pub struct ScopeSnapshot {
    pub sources: Vec<Source>,
    pub domains: Vec<Domain>,
    pub requirements: Vec<Requirement>,
    pub boundaries: Vec<Boundary>,
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
    pub fn graph_records(&self) -> GraphRecords<'_> {
        GraphRecords {
            sources: &self.sources,
            domains: &self.domains,
            requirements: &self.requirements,
            boundaries: &self.boundaries,
            topics: &self.topics,
            questions: &self.questions,
            resolutions: &self.resolutions,
            rules: &self.rules,
            verification_bindings: &self.verification_bindings,
            implementation_bindings: &self.implementation_bindings,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GraphRecords<'a> {
    pub sources: &'a [Source],
    pub domains: &'a [Domain],
    pub requirements: &'a [Requirement],
    pub boundaries: &'a [Boundary],
    pub topics: &'a [Topic],
    pub questions: &'a [Question],
    pub resolutions: &'a [Resolution],
    pub rules: &'a [Rule],
    pub verification_bindings: &'a [VerificationBinding],
    pub implementation_bindings: &'a [ImplementationBinding],
}

#[cfg(test)]
mod tests;
