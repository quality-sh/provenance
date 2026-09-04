use super::{compute_gaps, graph_query::GapGraph, model::GapItem};
use crate::{layout::ProvenanceLayout, state_store::StateStore};
use provenance_core::{
    Boundary, Question, Requirement, Resolution, Rule, ScopeId, Source, Thread, Topic,
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
    pub(in crate::cache) requirements: Vec<Requirement>,
    pub(in crate::cache) resolutions: Vec<Resolution>,
    pub(in crate::cache) rules: Vec<Rule>,
    pub(in crate::cache) topics: Vec<Topic>,
    pub(in crate::cache) questions: Vec<Question>,
    pub(in crate::cache) threads: Vec<Thread>,
    pub(in crate::cache) domains: Vec<provenance_core::Domain>,
    pub(in crate::cache) boundaries: Vec<provenance_core::Boundary>,
}

impl GraphRecords {
    /// Reads under a lock the caller already holds. Retired sources,
    /// requirements, and rules leave; a resolution whose every requirement
    /// retired leaves with them, and so do the topics, questions, and
    /// boundaries of a retired requirement. A reference from a live record
    /// to a retired record is dropped too: `check` reads the unfiltered
    /// lists and resolves it, so the gap report must not call it dangling.
    pub(in crate::cache) fn load(scope: &ScopeId, store: &StateStore) -> anyhow::Result<Self> {
        let mut sources = store.list_sources(scope)?;
        let mut requirements = store.list_requirements(scope)?;
        let mut rules = store.list_rules(scope)?;
        let mut resolutions = store.list_resolutions(scope)?;
        let mut topics = store.list_topics(scope)?;
        let mut questions = store.list_questions(scope)?;
        let mut boundaries = store.list_boundaries(scope)?;
        let retired_sources = retired_ids(&sources, |record| (&record.id, record.retired));
        let retired_requirements =
            retired_ids(&requirements, |record| (&record.id, record.retired));
        let retired_resolutions = resolutions
            .iter()
            .filter(|resolution| {
                !resolution.requirement_ids.is_empty()
                    && resolution
                        .requirement_ids
                        .iter()
                        .all(|id| retired_requirements.contains(id.as_str()))
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
        sources.retain(|record| !record.retired);
        requirements.retain(|record| !record.retired);
        rules.retain(|record| !record.retired);
        resolutions.retain(|record| !retired_resolutions.contains(record.id.as_str()));
        topics.retain(|record| !retired_topics.contains(record.id.as_str()));
        questions.retain(|record| !retired_questions.contains(record.id.as_str()));
        // A boundary's requirement is a required reference, so it cannot be
        // emptied; the boundary leaves with its requirement, like a topic.
        boundaries.retain(|record| !retired_requirements.contains(record.requirement_id.as_str()));
        let retired = RetiredNodes {
            sources: retired_sources,
            requirements: retired_requirements,
            resolutions: retired_resolutions,
        };
        scrub_sources(&retired, &mut sources);
        scrub_requirements(&retired, &mut requirements);
        scrub_resolutions(&retired, &mut resolutions);
        scrub_rules(&retired, &mut rules);
        scrub_boundaries(&retired, &mut boundaries);
        Ok(Self {
            sources,
            requirements,
            resolutions,
            rules,
            topics,
            questions,
            threads: store.list_threads(scope)?,
            domains: store.list_domains(scope)?,
            boundaries,
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

/// The retired records a live record's reference fields can name.
struct RetiredNodes {
    sources: BTreeSet<String>,
    requirements: BTreeSet<String>,
    resolutions: BTreeSet<String>,
}

fn scrub_sources(retired: &RetiredNodes, sources: &mut [Source]) {
    for source in sources {
        source
            .supersedes
            .retain(|id| !retired.sources.contains(id.as_str()));
    }
}

fn scrub_requirements(retired: &RetiredNodes, requirements: &mut [Requirement]) {
    for requirement in requirements {
        requirement
            .source_refs
            .retain(|reference| !retired.sources.contains(reference.source_id.as_str()));
        if let Some(id) = &requirement.refines {
            if retired.requirements.contains(id.as_str()) {
                requirement.refines = None;
            }
        }
        requirement
            .depends_on
            .retain(|id| !retired.requirements.contains(id.as_str()));
        requirement
            .supersedes
            .retain(|id| !retired.requirements.contains(id.as_str()));
        if let Some(id) = &requirement.spawned_by {
            if retired.resolutions.contains(id.as_str()) {
                requirement.spawned_by = None;
            }
        }
    }
}

fn scrub_resolutions(retired: &RetiredNodes, resolutions: &mut [Resolution]) {
    for resolution in resolutions {
        resolution
            .requirement_ids
            .retain(|id| !retired.requirements.contains(id.as_str()));
        resolution
            .supersedes
            .retain(|id| !retired.resolutions.contains(id.as_str()));
    }
}

fn scrub_rules(retired: &RetiredNodes, rules: &mut [Rule]) {
    for rule in rules {
        rule.requirement_ids
            .retain(|id| !retired.requirements.contains(id.as_str()));
        rule.resolution_ids
            .retain(|id| !retired.resolutions.contains(id.as_str()));
    }
}

fn scrub_boundaries(retired: &RetiredNodes, boundaries: &mut [Boundary]) {
    for boundary in boundaries {
        if let Some(reference) = &boundary.source_ref {
            if retired.sources.contains(reference.source_id.as_str()) {
                boundary.source_ref = None;
            }
        }
    }
}
