use super::{compute_gaps, graph_query::GapGraph, model::GapItem};
use crate::{layout::ProvenanceLayout, state_store::StateStore};
use provenance_core::{
    Contribution, Question, Requirement, Resolution, Rule, ScopeId, Source, SynthesisPacket,
    Thread, Topic,
};

pub fn find_gaps(layout: &ProvenanceLayout, scope: &ScopeId) -> anyhow::Result<Vec<GapItem>> {
    let store = StateStore::new(layout.clone());
    store.with_repository_publication(|| find_gaps_locked(scope, &store))
}

fn find_gaps_locked(scope: &ScopeId, store: &StateStore) -> anyhow::Result<Vec<GapItem>> {
    Ok(compute_gaps(
        &GraphRecords::load(scope, store)?.graph(scope),
    ))
}

/// Every record a [`GapGraph`] borrows, owned. Readers that need graph joins
/// rather than the gap list itself load the scope once through here and then
/// build the same graph the gap policy sees, so the two cannot drift.
pub(in crate::cache) struct GraphRecords {
    pub(in crate::cache) sources: Vec<Source>,
    pub(in crate::cache) requirements: Vec<Requirement>,
    pub(in crate::cache) resolutions: Vec<Resolution>,
    pub(in crate::cache) rules: Vec<Rule>,
    pub(in crate::cache) topics: Vec<Topic>,
    pub(in crate::cache) questions: Vec<Question>,
    pub(in crate::cache) threads: Vec<Thread>,
    pub(in crate::cache) domains: Vec<provenance_core::Domain>,
    pub(in crate::cache) boundaries: Vec<provenance_core::Boundary>,
    pub(in crate::cache) contributions: Vec<Contribution>,
    pub(in crate::cache) synthesis_packets: Vec<SynthesisPacket>,
}

impl GraphRecords {
    /// Reads the canonical records under the caller's lock.
    pub(in crate::cache) fn load(scope: &ScopeId, store: &StateStore) -> anyhow::Result<Self> {
        Ok(Self {
            sources: store.list_sources(scope)?,
            requirements: store.list_requirements(scope)?,
            resolutions: store.list_resolutions(scope)?,
            rules: store.list_rules(scope)?,
            topics: store.list_topics(scope)?,
            questions: store.list_questions(scope)?,
            threads: store.list_threads(scope)?,
            domains: store.list_domains(scope)?,
            boundaries: store.list_boundaries(scope)?,
            contributions: store.list_contributions(scope)?,
            synthesis_packets: store.list_synthesis_packets(scope)?,
        })
    }

    pub(in crate::cache) fn graph<'a>(&'a self, scope: &'a ScopeId) -> GapGraph<'a> {
        GapGraph {
            scope,
            sources: &self.sources,
            requirements: &self.requirements,
            resolutions: &self.resolutions,
            rules: &self.rules,
            topics: &self.topics,
            questions: &self.questions,
            threads: &self.threads,
            domains: &self.domains,
            boundaries: &self.boundaries,
            contributions: &self.contributions,
            synthesis_packets: &self.synthesis_packets,
        }
    }
}
