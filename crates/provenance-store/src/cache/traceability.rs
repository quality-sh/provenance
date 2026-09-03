use crate::layout::ProvenanceLayout;
use crate::state_store::StateStore;
use provenance_core::model::relations::RelationRow;
use provenance_core::{NodeType, Requirement, Resolution, Rule, Source, StableId};

/// One rule and the chain behind it.
///
/// `relations` holds only the rows the walk crossed: the rule's
/// `requirement_ids` and `resolution_ids`, its resolutions'
/// `requirement_ids`, and the citations of every requirement reached.
#[derive(Debug, serde::Serialize)]
pub struct TraceabilityView {
    pub rule: Rule,
    pub resolutions: Vec<Resolution>,
    pub requirements: Vec<Requirement>,
    pub sources: Vec<Source>,
    pub relations: Vec<RelationRow>,
}

pub fn trace_rule(
    layout: &ProvenanceLayout,
    scope: &provenance_core::ScopeId,
    rule_id: &provenance_core::StableId,
) -> anyhow::Result<TraceabilityView> {
    let store = StateStore::new(layout.clone());
    store.with_repository_publication(|| trace_rule_locked(scope, rule_id, &store))
}

fn row(owner: (NodeType, &StableId), relation: &str, target: (NodeType, &StableId)) -> RelationRow {
    RelationRow {
        owner_type: owner.0,
        owner_id: owner.1.clone(),
        relation: relation.to_string(),
        target_type: target.0,
        target_id: target.1.clone(),
    }
}

fn trace_rule_locked(
    scope: &provenance_core::ScopeId,
    rule_id: &provenance_core::StableId,
    store: &StateStore,
) -> anyhow::Result<TraceabilityView> {
    let rule = store
        .list_rules(scope)?
        .into_iter()
        .find(|rule| rule.id == *rule_id)
        .ok_or_else(|| anyhow::anyhow!("rule not found"))?;
    let mut relations = Vec::new();
    let resolutions: Vec<Resolution> = store
        .list_resolutions(scope)?
        .into_iter()
        .filter(|resolution| rule.resolution_ids.contains(&resolution.id))
        .collect();
    for id in &rule.resolution_ids {
        relations.push(row(
            (NodeType::Rule, &rule.id),
            "resolution_ids",
            (NodeType::Resolution, id),
        ));
    }
    let mut requirement_ids = rule.requirement_ids.clone();
    for id in &rule.requirement_ids {
        relations.push(row(
            (NodeType::Rule, &rule.id),
            "requirement_ids",
            (NodeType::Requirement, id),
        ));
    }
    for resolution in &resolutions {
        for id in &resolution.requirement_ids {
            relations.push(row(
                (NodeType::Resolution, &resolution.id),
                "requirement_ids",
                (NodeType::Requirement, id),
            ));
            if !requirement_ids.contains(id) {
                requirement_ids.push(id.clone());
            }
        }
    }
    let requirements: Vec<Requirement> = store
        .list_requirements(scope)?
        .into_iter()
        .filter(|requirement| requirement_ids.contains(&requirement.id))
        .collect();
    let mut source_ids = Vec::new();
    for requirement in &requirements {
        for reference in &requirement.source_refs {
            relations.push(row(
                (NodeType::Requirement, &requirement.id),
                "cites",
                (NodeType::Source, &reference.source_id),
            ));
            if !source_ids.contains(&reference.source_id) {
                source_ids.push(reference.source_id.clone());
            }
        }
    }
    let sources = store
        .list_sources(scope)?
        .into_iter()
        .filter(|source| source_ids.contains(&source.id))
        .collect();
    Ok(TraceabilityView {
        rule,
        resolutions,
        requirements,
        sources,
        relations,
    })
}
