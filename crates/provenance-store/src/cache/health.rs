use crate::cache::find_gaps;
use crate::cache::gaps::{GraphQuery, GraphRecords};
use crate::layout::ProvenanceLayout;
use crate::state_store::StateStore;
use provenance_core::RequirementStatus;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEvidenceReference {
    pub subject_id: String,
    pub document: String,
    pub section: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEvidence {
    pub rule_ids: BTreeSet<String>,
    pub references: Vec<GraphEvidenceReference>,
    pub verification_bindings: Vec<provenance_core::VerificationBinding>,
    pub implementation_bindings: Vec<provenance_core::ImplementationBinding>,
}

#[derive(Debug, serde::Serialize)]
pub struct CountMetric {
    pub total: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct RuleHealthMetric {
    pub total: usize,
    pub with_complete_traceability: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct HealthView {
    pub requirements: CountMetric,
    pub source_linked_requirements: usize,
    pub resolved_requirements: usize,
    pub requirements_with_rules: usize,
    pub rules: RuleHealthMetric,
    pub gaps: CountMetric,
}

#[derive(Debug, serde::Serialize)]
pub struct OrphanRuleItem {
    pub rule_id: String,
    pub missing: Vec<String>,
}

/// Loads every canonical graph reference that names a repository path.
///
/// Rule source documents are direct citations. A Source becomes a citation
/// only when a Requirement cites it; unreferenced catalog entries are not
/// evidence for anything in the graph. Path syntax is interpreted by the
/// command layer.
///
/// Active views leave retired records and retired bindings out. A caller
/// that asks for retired ones gets the history alongside what stands today.
pub fn graph_evidence(
    layout: &ProvenanceLayout,
    scope: &provenance_core::ScopeId,
    include_retired: bool,
) -> anyhow::Result<GraphEvidence> {
    let store = StateStore::new(layout.clone());
    store.with_repository_publication(|| graph_evidence_locked(scope, &store, include_retired))
}

fn graph_evidence_locked(
    scope: &provenance_core::ScopeId,
    store: &StateStore,
    include_retired: bool,
) -> anyhow::Result<GraphEvidence> {
    let requirements = store
        .list_requirements(scope)?
        .into_iter()
        .filter(|requirement| include_retired || !requirement.retired)
        .collect::<Vec<_>>();
    let cited_sources = requirements
        .iter()
        .flat_map(|requirement| &requirement.source_refs)
        .map(|reference| reference.source_id.as_str().to_string())
        .collect::<BTreeSet<_>>();
    let rules = store
        .list_rules(scope)?
        .into_iter()
        .filter(|rule| include_retired || !rule.retired)
        .collect::<Vec<_>>();
    let rule_ids = rules
        .iter()
        .map(|rule| rule.id.as_str().to_string())
        .collect::<BTreeSet<_>>();
    let mut references = rules
        .into_iter()
        .filter_map(|rule| {
            rule.source_document.map(|document| GraphEvidenceReference {
                subject_id: rule.id.as_str().to_string(),
                document,
                section: rule.source_section,
            })
        })
        .collect::<Vec<_>>();
    references.extend(store.list_sources(scope)?.into_iter().filter_map(|source| {
        if source.retired && !include_retired {
            return None;
        }
        (cited_sources.contains(source.id.as_str()))
            .then_some(source.reference)
            .flatten()
            .map(|document| GraphEvidenceReference {
                subject_id: source.id.as_str().to_string(),
                document,
                section: None,
            })
    }));
    references.sort_by(|left, right| {
        (&left.subject_id, &left.document, &left.section).cmp(&(
            &right.subject_id,
            &right.document,
            &right.section,
        ))
    });
    references.dedup();
    Ok(GraphEvidence {
        verification_bindings: store
            .list_verification_bindings(scope)?
            .into_iter()
            .filter(|binding| include_retired || !binding.retired)
            .filter(|binding| rule_ids.contains(binding.rule_id.as_str()))
            .collect(),
        implementation_bindings: store
            .list_implementation_bindings(scope)?
            .into_iter()
            .filter(|binding| include_retired || !binding.retired)
            .filter(|binding| rule_ids.contains(binding.rule_id.as_str()))
            .collect(),
        rule_ids,
        references,
    })
}

pub fn coverage_health(
    layout: &ProvenanceLayout,
    scope: &provenance_core::ScopeId,
) -> anyhow::Result<HealthView> {
    let store = StateStore::new(layout.clone());
    store.with_repository_publication(|| coverage_health_locked(layout, scope, &store))
}

fn coverage_health_locked(
    layout: &ProvenanceLayout,
    scope: &provenance_core::ScopeId,
    store: &StateStore,
) -> anyhow::Result<HealthView> {
    let requirements = store
        .list_requirements(scope)?
        .into_iter()
        .filter(|requirement| !requirement.retired)
        .collect::<Vec<_>>();
    let rules = store
        .list_rules(scope)?
        .into_iter()
        .filter(|rule| !rule.retired)
        .collect::<Vec<_>>();
    let resolutions = store.list_resolutions(scope)?;
    let source_linked_requirements = requirements
        .iter()
        .filter(|req| !req.source_refs.is_empty())
        .count();
    let resolved_requirements = requirements
        .iter()
        .filter(|req| {
            req.status == RequirementStatus::Resolved
                || resolutions
                    .iter()
                    .any(|resolution| resolution.requirement_ids.contains(&req.id))
        })
        .count();
    let requirements_with_rules = requirements
        .iter()
        .filter(|req| {
            rules
                .iter()
                .any(|rule| rule.requirement_ids.contains(&req.id))
        })
        .count();
    let orphan_count = orphan_rules(layout, scope)?.len();
    let with_complete_traceability = rules.len().saturating_sub(orphan_count);
    let gaps = find_gaps(layout, scope)?.len();
    Ok(HealthView {
        requirements: CountMetric {
            total: requirements.len(),
        },
        source_linked_requirements,
        resolved_requirements,
        requirements_with_rules,
        rules: RuleHealthMetric {
            total: rules.len(),
            with_complete_traceability,
        },
        gaps: CountMetric { total: gaps },
    })
}

/// Rules whose trace back to a source is incomplete.
///
/// No source reaches the rule through a requirement it names. The
/// requirement itself is required by the record type, so `missing` only
/// ever names the source.
pub fn orphan_rules(
    layout: &ProvenanceLayout,
    scope: &provenance_core::ScopeId,
) -> anyhow::Result<Vec<OrphanRuleItem>> {
    let store = StateStore::new(layout.clone());
    store.with_repository_publication(|| orphan_rules_locked(scope, &store))
}

fn orphan_rules_locked(
    scope: &provenance_core::ScopeId,
    store: &StateStore,
) -> anyhow::Result<Vec<OrphanRuleItem>> {
    let records = GraphRecords::load(scope, store)?;
    let graph = records.graph(scope);
    let query = GraphQuery::new(&graph);
    Ok(records
        .rules
        .iter()
        .filter(|rule| !query.rule_trace_reaches_source(&rule.id))
        .map(|rule| OrphanRuleItem {
            rule_id: rule.id.as_str().to_string(),
            missing: vec!["source".to_string()],
        })
        .collect())
}
