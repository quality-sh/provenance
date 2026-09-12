use camino::Utf8Path;
use provenance_core::{
    AssertionRecord, Boundary, Contribution, DispositionRecord, Domain, ImplementationBinding,
    Message, ProposalCard, Question, Requirement, Resolution, Rule, ScopeId, Source, StableId,
    SynthesisPacket, Thread, Topic, VerificationBinding,
};
use provenance_store::{layout::ProvenanceLayout, state_store::StateStore};

#[derive(Debug)]
pub(super) struct Store {
    layout: ProvenanceLayout,
    state: StateStore,
}

impl Store {
    pub(super) fn open(repo: impl AsRef<Utf8Path>) -> Self {
        let layout = ProvenanceLayout::new(repo.as_ref().to_path_buf());
        let state = StateStore::new(layout.clone());
        Self { layout, state }
    }

    pub(super) const fn layout(&self) -> &ProvenanceLayout {
        &self.layout
    }

    pub(super) fn requirement(
        &self,
        scope: &ScopeId,
        id: &StableId,
    ) -> anyhow::Result<Requirement> {
        self.list_requirements(scope)?
            .into_iter()
            .find(|record| record.id == *id)
            .ok_or_else(|| anyhow::Error::new(NotFound::Requirement))
    }

    pub(super) fn rule(&self, scope: &ScopeId, id: &StableId) -> anyhow::Result<Rule> {
        self.list_rules(scope)?
            .into_iter()
            .find(|record| record.id == *id)
            .ok_or_else(|| anyhow::Error::new(NotFound::Rule(id.clone())))
    }

    #[allow(dead_code)]
    pub(super) fn source(&self, scope: &ScopeId, id: &StableId) -> anyhow::Result<Source> {
        self.list_sources(scope)?
            .into_iter()
            .find(|record| record.id == *id)
            .ok_or_else(|| anyhow::Error::new(NotFound::Source))
    }

    pub(super) fn snapshot(&self, scope: &ScopeId) -> anyhow::Result<ScopeSnapshot> {
        self.with_repository_publication(|| {
            Ok(ScopeSnapshot {
                sources: self.list_sources(scope)?,
                domains: self.list_domains(scope)?,
                requirements: self.list_requirements(scope)?,
                boundaries: self.list_boundaries(scope)?,
                topics: self.list_topics(scope)?,
                questions: self.list_questions(scope)?,
                resolutions: self.list_resolutions(scope)?,
                rules: self.list_rules(scope)?,
                verification_bindings: self.list_verification_bindings(scope)?,
                implementation_bindings: self.list_implementation_bindings(scope)?,
                threads: self.list_threads(scope)?,
                messages: self.list_messages(scope)?,
                contributions: self.list_contributions(scope)?,
                synthesis_packets: self.list_synthesis_packets(scope)?,
                proposal_cards: self.list_proposal_definitions(scope)?,
                assertion_records: self.list_assertion_records(scope)?,
                dispositions: self.list_dispositions(scope)?,
            })
        })
    }
}

impl std::ops::Deref for Store {
    type Target = StateStore;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum NotFound {
    Requirement,
    Rule(StableId),
    Source,
}

impl std::fmt::Display for NotFound {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Requirement => formatter.write_str("requirement does not exist"),
            Self::Rule(id) => write!(formatter, "rule `{}` not found in scope", id.as_str()),
            Self::Source => formatter.write_str("source does not exist"),
        }
    }
}

impl std::error::Error for NotFound {}

#[derive(Debug)]
pub(super) struct ScopeSnapshot {
    pub(super) sources: Vec<Source>,
    pub(super) domains: Vec<Domain>,
    pub(super) requirements: Vec<Requirement>,
    pub(super) boundaries: Vec<Boundary>,
    pub(super) topics: Vec<Topic>,
    pub(super) questions: Vec<Question>,
    pub(super) resolutions: Vec<Resolution>,
    pub(super) rules: Vec<Rule>,
    pub(super) verification_bindings: Vec<VerificationBinding>,
    pub(super) implementation_bindings: Vec<ImplementationBinding>,
    pub(super) threads: Vec<Thread>,
    pub(super) messages: Vec<Message>,
    pub(super) contributions: Vec<Contribution>,
    pub(super) synthesis_packets: Vec<SynthesisPacket>,
    pub(super) proposal_cards: Vec<ProposalCard>,
    pub(super) assertion_records: Vec<AssertionRecord>,
    pub(super) dispositions: Vec<DispositionRecord>,
}

impl ScopeSnapshot {
    pub(super) fn graph_records(&self) -> GraphRecords<'_> {
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
pub(super) struct GraphRecords<'a> {
    pub(super) sources: &'a [Source],
    pub(super) domains: &'a [Domain],
    pub(super) requirements: &'a [Requirement],
    pub(super) boundaries: &'a [Boundary],
    pub(super) topics: &'a [Topic],
    pub(super) questions: &'a [Question],
    pub(super) resolutions: &'a [Resolution],
    pub(super) rules: &'a [Rule],
    pub(super) verification_bindings: &'a [VerificationBinding],
    pub(super) implementation_bindings: &'a [ImplementationBinding],
}

#[cfg(test)]
mod tests;
