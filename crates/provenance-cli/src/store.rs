use camino::Utf8Path;
use provenance_core::{
    AssertionRecord, Boundary, Contribution, DispositionRecord, Domain, ImplementationBinding,
    Message, ProposalCard, Question, Requirement, Resolution, Rule, ScopeId, Source, StableId,
    SynthesisPacket, Thread, Topic, VerificationBinding,
};
use provenance_store::{layout::ProvenanceLayout, state_store::StateStore};

#[derive(Debug)]
pub(crate) struct Store {
    layout: ProvenanceLayout,
    state: StateStore,
}

impl Store {
    pub(crate) fn open(repo: impl AsRef<Utf8Path>) -> Self {
        let layout = ProvenanceLayout::new(repo.as_ref().to_path_buf());
        let state = StateStore::new(layout.clone());
        Self { layout, state }
    }

    pub(crate) const fn layout(&self) -> &ProvenanceLayout {
        &self.layout
    }

    pub(crate) fn requirement(
        &self,
        scope: &ScopeId,
        id: &StableId,
    ) -> anyhow::Result<Requirement> {
        self.list_requirements(scope)?
            .into_iter()
            .find(|record| record.id == *id)
            .ok_or_else(|| anyhow::Error::new(NotFound::Requirement))
    }

    pub(crate) fn rule(&self, scope: &ScopeId, id: &StableId) -> anyhow::Result<Rule> {
        self.list_rules(scope)?
            .into_iter()
            .find(|record| record.id == *id)
            .ok_or_else(|| anyhow::Error::new(NotFound::Rule(id.clone())))
    }

    #[allow(dead_code)]
    pub(crate) fn source(&self, scope: &ScopeId, id: &StableId) -> anyhow::Result<Source> {
        self.list_sources(scope)?
            .into_iter()
            .find(|record| record.id == *id)
            .ok_or_else(|| anyhow::Error::new(NotFound::Source))
    }

    pub(crate) fn snapshot(&self, scope: &ScopeId) -> anyhow::Result<ScopeSnapshot> {
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
pub(crate) enum NotFound {
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
pub(crate) struct ScopeSnapshot {
    pub(crate) sources: Vec<Source>,
    pub(crate) domains: Vec<Domain>,
    pub(crate) requirements: Vec<Requirement>,
    pub(crate) boundaries: Vec<Boundary>,
    pub(crate) topics: Vec<Topic>,
    pub(crate) questions: Vec<Question>,
    pub(crate) resolutions: Vec<Resolution>,
    pub(crate) rules: Vec<Rule>,
    pub(crate) verification_bindings: Vec<VerificationBinding>,
    pub(crate) implementation_bindings: Vec<ImplementationBinding>,
    pub(crate) threads: Vec<Thread>,
    pub(crate) messages: Vec<Message>,
    pub(crate) contributions: Vec<Contribution>,
    pub(crate) synthesis_packets: Vec<SynthesisPacket>,
    pub(crate) proposal_cards: Vec<ProposalCard>,
    pub(crate) assertion_records: Vec<AssertionRecord>,
    pub(crate) dispositions: Vec<DispositionRecord>,
}

impl ScopeSnapshot {
    pub(crate) const fn graph_records(&self) -> GraphRecords<'_> {
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
pub(crate) struct GraphRecords<'a> {
    pub(crate) sources: &'a [Source],
    pub(crate) domains: &'a [Domain],
    pub(crate) requirements: &'a [Requirement],
    pub(crate) boundaries: &'a [Boundary],
    pub(crate) topics: &'a [Topic],
    pub(crate) questions: &'a [Question],
    pub(crate) resolutions: &'a [Resolution],
    pub(crate) rules: &'a [Rule],
    pub(crate) verification_bindings: &'a [VerificationBinding],
    pub(crate) implementation_bindings: &'a [ImplementationBinding],
}

#[cfg(test)]
mod tests;
