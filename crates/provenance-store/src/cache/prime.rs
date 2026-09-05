use crate::cache::{find_gaps, GapItem};
use crate::layout::ProvenanceLayout;
use crate::state_store::StateStore;
use provenance_core::model::relations::{
    related_nodes, RecordFront, RelationDirection, RelationRow,
};
use provenance_core::{Message, NodeType, Requirement, Rule, Source, Thread};
use std::collections::BTreeSet;
use std::fmt::Write;

#[derive(Debug, serde::Serialize)]
pub struct RequirementGraphView {
    pub requirement: Requirement,
    pub sources: Vec<Source>,
    pub relations: Vec<RelationRow>,
}

#[derive(Debug, serde::Serialize)]
pub struct PrimeThreadView {
    pub thread: Thread,
    pub messages: Vec<Message>,
}

#[derive(Debug, serde::Serialize)]
pub struct PrimeContextView {
    pub scope_id: String,
    pub rules: Vec<Rule>,
    pub gaps: Vec<GapItem>,
    pub threads: Vec<PrimeThreadView>,
}

pub fn prime_context(
    layout: &ProvenanceLayout,
    scope: &provenance_core::ScopeId,
    include_threads: bool,
) -> anyhow::Result<PrimeContextView> {
    let store = StateStore::new(layout.clone());
    store.with_repository_publication(|| {
        prime_context_locked(layout, scope, include_threads, &store)
    })
}

fn prime_context_locked(
    layout: &ProvenanceLayout,
    scope: &provenance_core::ScopeId,
    include_threads: bool,
    store: &StateStore,
) -> anyhow::Result<PrimeContextView> {
    let threads = if include_threads {
        let messages = store.list_messages(scope)?;
        store
            .list_threads(scope)?
            .into_iter()
            .filter(|thread| thread.status == provenance_core::ThreadStatus::Active)
            .map(|thread| PrimeThreadView {
                messages: messages
                    .iter()
                    .filter(|message| message.thread_id == thread.id)
                    .cloned()
                    .collect(),
                thread,
            })
            .collect()
    } else {
        Vec::new()
    };
    Ok(PrimeContextView {
        scope_id: scope.as_str().to_string(),
        rules: store
            .list_rules(scope)?
            .into_iter()
            .filter(|rule| !rule.retired)
            .collect(),
        gaps: find_gaps(layout, scope)?,
        threads,
    })
}

pub fn render_prime_markdown(view: &PrimeContextView) -> String {
    let mut out = format!(
        "# Provenance Prime\n\nScope: {}\n\n## Rules\n",
        view.scope_id
    );
    for rule in &view.rules {
        let _ = writeln!(out, "- {}", rule.id.as_str());
    }
    out.push_str("\n## Gaps\n");
    if view.gaps.is_empty() {
        out.push_str("- none\n");
    }
    for gap in &view.gaps {
        let _ = writeln!(out, "- {}: {}", gap.subject(), gap.reason);
    }
    out.push_str("\n## Threads\n");
    for item in &view.threads {
        let _ = writeln!(
            out,
            "- {} on {}",
            item.thread.id.as_str(),
            item.thread.parent.node_id.as_str()
        );
        for message in &item.messages {
            let _ = writeln!(out, "  - {}: {}", message.id.as_str(), message.body);
        }
    }
    out
}

pub fn get_requirement_graph(
    layout: &ProvenanceLayout,
    scope: &provenance_core::ScopeId,
    requirement_id: &provenance_core::StableId,
) -> anyhow::Result<RequirementGraphView> {
    let store = StateStore::new(layout.clone());
    store
        .with_repository_publication(|| get_requirement_graph_locked(scope, requirement_id, &store))
}

fn get_requirement_graph_locked(
    scope: &provenance_core::ScopeId,
    requirement_id: &provenance_core::StableId,
    store: &StateStore,
) -> anyhow::Result<RequirementGraphView> {
    let requirement = store
        .list_requirements(scope)?
        .into_iter()
        .find(|requirement| requirement.id == *requirement_id)
        .ok_or_else(|| anyhow::anyhow!("requirement not found"))?;
    let source_ids: BTreeSet<_> = requirement
        .source_refs
        .iter()
        .map(|reference| reference.source_id.as_str().to_string())
        .collect();
    let all_sources = store.list_sources(scope)?;
    let requirements = store.list_requirements(scope)?;
    let resolutions = store.list_resolutions(scope)?;
    let rules = store.list_rules(scope)?;
    let topics = store.list_topics(scope)?;
    let questions = store.list_questions(scope)?;
    let domains = store.list_domains(scope)?;
    let boundaries = store.list_boundaries(scope)?;
    let front = RecordFront {
        sources: &all_sources,
        requirements: &requirements,
        resolutions: &resolutions,
        rules: &rules,
        topics: &topics,
        questions: &questions,
        domains: &domains,
        boundaries: &boundaries,
    };
    let relations = related_nodes(&front, NodeType::Requirement, requirement_id)
        .into_iter()
        .map(|node| match node.direction {
            RelationDirection::Out => RelationRow {
                owner_type: NodeType::Requirement,
                owner_id: requirement_id.clone(),
                relation: node.relation.to_string(),
                target_type: node.endpoint.node_type,
                target_id: node.endpoint.id,
            },
            RelationDirection::In => RelationRow {
                owner_type: node.endpoint.node_type,
                owner_id: node.endpoint.id,
                relation: node.relation.to_string(),
                target_type: NodeType::Requirement,
                target_id: requirement_id.clone(),
            },
        })
        .collect();
    let sources = all_sources
        .into_iter()
        .filter(|source| source_ids.contains(source.id.as_str()))
        .collect();
    Ok(RequirementGraphView {
        requirement,
        sources,
        relations,
    })
}
