use camino::Utf8Path;
use provenance_core::{EdgeType, NodeType, StableId};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{ReconcileState, StateStore, TypedResourceKind, TypedSpecInput, TypedSpecResult},
};
use serde::Serialize;
use std::collections::BTreeMap;

use super::sites;

mod evidence;
mod human;

pub(super) use human::render;

#[derive(Serialize)]
pub(super) struct TypedSpecPlan {
    #[serde(flatten)]
    reconciliation: TypedSpecResult,
    affected_rules: Vec<AffectedRule>,
}

#[derive(Serialize)]
pub(super) struct AffectedRule {
    #[serde(flatten)]
    rule: provenance_core::protocol::AffectedRule,
    evidence: evidence::RuleEvidence,
}

impl AffectedRule {
    pub(super) const fn id(&self) -> &StableId {
        &self.rule.id
    }

    pub(super) fn implementations(&self) -> &[provenance_core::protocol::ImplementationSite] {
        &self.rule.implementations
    }

    pub(super) fn verifications(&self) -> &[provenance_core::protocol::VerificationSite] {
        &self.rule.verifications
    }
}

pub(super) fn typed_spec(
    repo: &Utf8Path,
    scope: &provenance_core::ScopeId,
    input: TypedSpecInput,
) -> anyhow::Result<TypedSpecPlan> {
    let store = StateStore::new(ProvenanceLayout::new(repo.to_path_buf()));
    let reconciliation = store.plan_typed_spec(scope, input)?;
    let reviews = evidence::reviews(&store, scope, &reconciliation)?;
    let mut changed_rules = affected_rule_ids(&store, scope, &reconciliation)?;
    changed_rules.extend(reviews.rules.iter().cloned());
    changed_rules.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    changed_rules.dedup();
    let scans = provenance_scanner::scan_path(repo)?;
    let bindings = store.active_verification_bindings(scope)?;
    let implementation_changes = reconciliation
        .resources
        .iter()
        .filter(|resource| {
            resource.kind == TypedResourceKind::Rule
                && resource
                    .changes
                    .iter()
                    .any(|change| change.field == "implementation")
        })
        .map(|resource| &resource.id)
        .collect::<Vec<_>>();
    let implementations = store
        .active_implementation_bindings(scope)?
        .into_iter()
        .filter(|binding| !implementation_changes.contains(&&binding.rule_id))
        .chain(reconciliation.implementation_bindings.iter().cloned())
        .map(|binding| (binding.id.as_str().to_string(), binding))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .collect::<Vec<_>>();
    let evidence = sites::Evidence {
        scans: &scans,
        verifications: &bindings,
        implementations: &implementations,
    };
    let affected_rules = changed_rules
        .into_iter()
        .map(|id| AffectedRule {
            evidence: reviews.evidence(&id),
            rule: evidence.affected_rule(repo, id),
        })
        .collect();
    Ok(TypedSpecPlan {
        reconciliation,
        affected_rules,
    })
}

fn affected_rule_ids(
    store: &StateStore,
    scope: &provenance_core::ScopeId,
    reconciliation: &TypedSpecResult,
) -> anyhow::Result<Vec<StableId>> {
    let changed = reconciliation
        .resources
        .iter()
        .filter(|resource| resource.state != ReconcileState::Unchanged)
        .collect::<Vec<_>>();
    let changed_requirements = changed
        .iter()
        .filter(|resource| resource.kind == TypedResourceKind::Requirement)
        .map(|resource| &resource.id)
        .collect::<Vec<_>>();
    let mut rules = changed
        .iter()
        .filter(|resource| resource.kind == TypedResourceKind::Rule)
        .map(|resource| resource.id.clone())
        .collect::<Vec<_>>();
    let existing_implementations = store.active_implementation_bindings(scope)?;
    for binding in &reconciliation.implementation_bindings {
        if !existing_implementations
            .iter()
            .any(|existing| existing == binding)
        {
            push_unique(&mut rules, binding.rule_id.clone());
        }
    }

    for edge in store.list_edges()?.into_iter().filter(|edge| {
        edge.scope_id == *scope
            && edge.edge_type == EdgeType::Produces
            && edge.from_type == NodeType::Requirement
            && edge.to_type == NodeType::Rule
            && changed_requirements.contains(&&edge.from_id)
    }) {
        push_unique(&mut rules, edge.to_id);
    }
    for requirement in changed
        .iter()
        .filter(|resource| resource.kind == TypedResourceKind::Requirement)
    {
        for rule in reconciliation.resources.iter().filter(|resource| {
            resource.kind == TypedResourceKind::Rule
                && resource.parent.as_deref() == Some(requirement.key.as_str())
        }) {
            push_unique(&mut rules, rule.id.clone());
        }
    }
    Ok(rules)
}

fn push_unique(ids: &mut Vec<StableId>, id: StableId) {
    if !ids.contains(&id) {
        ids.push(id);
    }
}
