use super::GuardedStore;
use provenance_core::{
    AssertionRecord, Boundary, Contribution, DispositionRecord, Domain, ImplementationBinding,
    Manifest, Message, ProposalCard, Question, Requirement, RequirementReview, Resolution, Rule,
    ScopeId, Source, SynthesisPacket, Thread, Topic, VerificationBinding,
};

impl GuardedStore<'_> {
    pub fn manifest(&self) -> anyhow::Result<Manifest> {
        self.store.manifest()
    }

    pub fn list_scope_directories(&self) -> anyhow::Result<Vec<String>> {
        self.store.list_scope_directories()
    }

    pub fn list_sources(&self, scope: &ScopeId) -> anyhow::Result<Vec<Source>> {
        self.store.list_sources(scope)
    }

    pub fn list_requirements(&self, scope: &ScopeId) -> anyhow::Result<Vec<Requirement>> {
        self.store.list_requirements(scope)
    }

    pub fn list_requirement_reviews(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<RequirementReview>> {
        self.store.list_requirement_reviews(scope)
    }

    pub fn list_domains(&self, scope: &ScopeId) -> anyhow::Result<Vec<Domain>> {
        self.store.list_domains(scope)
    }

    pub fn list_boundaries(&self, scope: &ScopeId) -> anyhow::Result<Vec<Boundary>> {
        self.store.list_boundaries(scope)
    }

    pub fn list_topics(&self, scope: &ScopeId) -> anyhow::Result<Vec<Topic>> {
        self.store.list_topics(scope)
    }

    pub fn list_questions(&self, scope: &ScopeId) -> anyhow::Result<Vec<Question>> {
        self.store.list_questions(scope)
    }

    pub fn list_resolutions(&self, scope: &ScopeId) -> anyhow::Result<Vec<Resolution>> {
        self.store.list_resolutions(scope)
    }

    pub fn list_rules(&self, scope: &ScopeId) -> anyhow::Result<Vec<Rule>> {
        self.store.list_rules(scope)
    }

    pub fn list_verification_bindings(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<VerificationBinding>> {
        self.store.list_verification_bindings(scope)
    }

    pub fn active_verification_bindings(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<VerificationBinding>> {
        self.store.active_verification_bindings(scope)
    }

    pub fn list_implementation_bindings(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<ImplementationBinding>> {
        self.store.list_implementation_bindings(scope)
    }

    pub fn active_implementation_bindings(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<ImplementationBinding>> {
        self.store.active_implementation_bindings(scope)
    }

    pub fn list_threads(&self, scope: &ScopeId) -> anyhow::Result<Vec<Thread>> {
        self.store.list_threads(scope)
    }

    pub fn list_messages(&self, scope: &ScopeId) -> anyhow::Result<Vec<Message>> {
        self.store.list_messages(scope)
    }

    pub fn list_contributions(&self, scope: &ScopeId) -> anyhow::Result<Vec<Contribution>> {
        self.store.list_contributions(scope)
    }

    pub fn list_synthesis_packets(&self, scope: &ScopeId) -> anyhow::Result<Vec<SynthesisPacket>> {
        self.store.list_synthesis_packets(scope)
    }

    pub fn list_proposal_cards(&self, scope: &ScopeId) -> anyhow::Result<Vec<ProposalCard>> {
        self.store.list_proposal_cards(scope)
    }

    pub fn list_proposal_definitions(&self, scope: &ScopeId) -> anyhow::Result<Vec<ProposalCard>> {
        self.store.list_proposal_definitions(scope)
    }

    pub fn list_dispositions(&self, scope: &ScopeId) -> anyhow::Result<Vec<DispositionRecord>> {
        self.store.list_dispositions(scope)
    }

    pub fn list_assertion_records(&self, scope: &ScopeId) -> anyhow::Result<Vec<AssertionRecord>> {
        self.store.list_assertion_records(scope)
    }

    pub fn list_proposal_cards_with_actor_ids(
        &self,
        scope: &ScopeId,
        disposition_actor_ids: &[String],
    ) -> anyhow::Result<Vec<ProposalCard>> {
        self.store
            .list_proposal_cards_with_actor_ids(scope, disposition_actor_ids)
    }

    pub fn validate_ideation_scope(&self, scope: &ScopeId) -> anyhow::Result<()> {
        self.store.validate_ideation_scope(scope)
    }

    pub fn validate_ideation_scope_with_actor_ids(
        &self,
        scope: &ScopeId,
        disposition_actor_ids: &[String],
    ) -> anyhow::Result<()> {
        self.store
            .validate_ideation_scope_with_actor_ids(scope, disposition_actor_ids)
    }

    pub fn validate_graph_scope(&self, scope: &ScopeId) -> anyhow::Result<()> {
        self.store.validate_graph_scope(scope)
    }
}
