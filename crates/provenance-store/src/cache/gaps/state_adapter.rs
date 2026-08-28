use super::{compute_gaps, graph_query::GapGraph, model::GapItem};
use crate::{layout::ProvenanceLayout, state_store::StateStore};
use provenance_core::{
    Edge, IdeationTarget, NodeType, Question, Requirement, Resolution, Rule, ScopeId, Source,
    Thread, Topic,
};
use std::collections::BTreeSet;

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
    pub(in crate::cache) domains: Vec<provenance_core::Domain>,
    pub(in crate::cache) boundaries: Vec<provenance_core::Boundary>,
    pub(in crate::cache) requirements: Vec<Requirement>,
    pub(in crate::cache) resolutions: Vec<Resolution>,
    pub(in crate::cache) rules: Vec<Rule>,
    pub(in crate::cache) topics: Vec<Topic>,
    pub(in crate::cache) questions: Vec<Question>,
    pub(in crate::cache) edges: Vec<Edge>,
    pub(in crate::cache) threads: Vec<Thread>,
    /// Ideation records whose targets the dangling scan checks.
    pub(in crate::cache) ideation_targets: Vec<(String, IdeationTarget)>,
}

impl GraphRecords {
    /// Reads under a lock the caller already holds.
    pub(in crate::cache) fn load(scope: &ScopeId, store: &StateStore) -> anyhow::Result<Self> {
        let mut sources = store.list_sources(scope)?;
        let mut requirements = store.list_requirements(scope)?;
        let mut rules = store.list_rules(scope)?;
        let mut resolutions = store.list_resolutions(scope)?;
        let mut topics = store.list_topics(scope)?;
        let mut questions = store.list_questions(scope)?;
        let mut edges = store.list_edges()?;
        let retired_sources = retired_ids(&sources, |record| (&record.id, record.retired));
        let retired_requirements =
            retired_ids(&requirements, |record| (&record.id, record.retired));
        let retired_rules = retired_ids(&rules, |record| (&record.id, record.retired));
        let retired_resolutions = resolutions
            .iter()
            .filter(|resolution| {
                let targets = edges.iter().filter(|edge| {
                    edge.scope_id == *scope
                        && edge.edge_type == provenance_core::EdgeType::Resolves
                        && edge.from_type == NodeType::Resolution
                        && edge.from_id == resolution.id
                        && edge.to_type == NodeType::Requirement
                });
                let targets = targets.collect::<Vec<_>>();
                !targets.is_empty()
                    && targets
                        .iter()
                        .all(|edge| retired_requirements.contains(edge.to_id.as_str()))
            })
            .map(|record| record.id.as_str().to_string())
            .collect::<BTreeSet<_>>();
        let retired_topics = topics
            .iter()
            .filter(|topic| retired_requirements.contains(topic.requirement_id.as_str()))
            .map(|topic| topic.id.as_str().to_string())
            .collect::<BTreeSet<_>>();
        let retired_questions = questions
            .iter()
            .filter(|question| {
                retired_requirements.contains(question.requirement_id.as_str())
                    || retired_topics.contains(question.topic_id.as_str())
            })
            .map(|question| question.id.as_str().to_string())
            .collect::<BTreeSet<_>>();
        let retired = RetiredNodes {
            sources: retired_sources,
            requirements: retired_requirements,
            rules: retired_rules,
            resolutions: retired_resolutions,
            topics: retired_topics,
            questions: retired_questions,
        };
        sources.retain(|record| !record.retired);
        requirements.retain(|record| !record.retired);
        rules.retain(|record| !record.retired);
        resolutions.retain(|record| !retired.resolutions.contains(record.id.as_str()));
        topics.retain(|record| !retired.topics.contains(record.id.as_str()));
        questions.retain(|record| !retired.questions.contains(record.id.as_str()));
        edges.retain(|edge| {
            !retired.contains(edge.from_type, edge.from_id.as_str())
                && !retired.contains(edge.to_type, edge.to_id.as_str())
        });
        let mut ideation_targets = Vec::new();
        for contribution in store.list_contributions(scope)? {
            ideation_targets.push((
                format!("contribution {}", contribution.id.as_str()),
                contribution.target.clone(),
            ));
        }
        for packet in store.list_synthesis_packets(scope)? {
            ideation_targets.push((
                format!("synthesis packet {}", packet.id.as_str()),
                packet.target.clone(),
            ));
        }
        for proposal in store.list_proposal_cards(scope)? {
            ideation_targets.push((
                format!("proposal {}", proposal.id.as_str()),
                proposal.traceability.target.clone(),
            ));
        }
        Ok(Self {
            sources,
            domains: store.list_domains(scope)?,
            boundaries: store.list_boundaries(scope)?,
            requirements,
            resolutions,
            rules,
            topics,
            questions,
            edges,
            threads: store.list_threads(scope)?,
            ideation_targets,
        })
    }

    pub(in crate::cache) fn graph<'a>(&'a self, scope: &'a ScopeId) -> GapGraph<'a> {
        GapGraph {
            scope,
            sources: &self.sources,
            domains: &self.domains,
            boundaries: &self.boundaries,
            requirements: &self.requirements,
            resolutions: &self.resolutions,
            rules: &self.rules,
            topics: &self.topics,
            questions: &self.questions,
            edges: &self.edges,
            threads: &self.threads,
            ideation_targets: &self.ideation_targets,
        }
    }
}

fn retired_ids<'a, T>(
    records: &'a [T],
    fields: impl Fn(&'a T) -> (&'a provenance_core::StableId, bool),
) -> BTreeSet<String> {
    records
        .iter()
        .filter_map(|record| {
            let (id, retired) = fields(record);
            retired.then(|| id.as_str().to_string())
        })
        .collect()
}

struct RetiredNodes {
    sources: BTreeSet<String>,
    requirements: BTreeSet<String>,
    rules: BTreeSet<String>,
    resolutions: BTreeSet<String>,
    topics: BTreeSet<String>,
    questions: BTreeSet<String>,
}

impl RetiredNodes {
    fn contains(&self, node_type: NodeType, id: &str) -> bool {
        match node_type {
            NodeType::Source => self.sources.contains(id),
            NodeType::Requirement => self.requirements.contains(id),
            NodeType::Rule => self.rules.contains(id),
            NodeType::Resolution => self.resolutions.contains(id),
            NodeType::Topic => self.topics.contains(id),
            NodeType::Question => self.questions.contains(id),
            NodeType::Domain | NodeType::Boundary => false,
        }
    }
}
