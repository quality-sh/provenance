use super::{compute_gaps, graph_query::GapGraph, model::GapItem};
use crate::{repository::Repository, state_store::StateStore};
use provenance_core::{
    Contribution, Question, Requirement, Resolution, Rule, ScopeId, Source, SynthesisPacket,
    Thread, Topic,
};

pub fn find_gaps(repository: &Repository) -> anyhow::Result<Vec<GapItem>> {
    let store = repository.store();
    store.with_repository_publication(|held| find_gaps_locked(repository.scope(), held))
}

fn find_gaps_locked(scope: &ScopeId, store: &StateStore) -> anyhow::Result<Vec<GapItem>> {
    Ok(compute_gaps(
        &GraphRecords::load(scope, store)?.graph(scope),
    ))
}

/// Every record a [`GapGraph`] borrows, owned. Readers that need graph joins
/// rather than the gap list itself load the scope once through here and then
/// build the same graph the gap policy sees, so the two cannot drift.
///
/// A caller that already holds the records builds this directly and needs no
/// store: [`GraphRecords::graph`] gives the gap graph, and its
/// [`GapGraph::front`] gives the relation front.
#[derive(Debug, Clone, Default)]
pub struct GraphRecords {
    pub sources: Vec<Source>,
    pub requirements: Vec<Requirement>,
    pub resolutions: Vec<Resolution>,
    pub rules: Vec<Rule>,
    pub topics: Vec<Topic>,
    pub questions: Vec<Question>,
    pub threads: Vec<Thread>,
    pub domains: Vec<provenance_core::Domain>,
    pub boundaries: Vec<provenance_core::Boundary>,
    pub contributions: Vec<Contribution>,
    pub synthesis_packets: Vec<SynthesisPacket>,
}

impl GraphRecords {
    /// Reads the canonical records under the caller's lock.
    pub fn load(scope: &ScopeId, store: &StateStore) -> anyhow::Result<Self> {
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

    pub fn graph<'a>(&'a self, scope: &'a ScopeId) -> GapGraph<'a> {
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
