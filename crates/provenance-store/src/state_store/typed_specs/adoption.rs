//! Exact, per-target authorization for one-time declaration adoption.

mod implementation;
mod relationships;

use std::collections::BTreeSet;

use provenance_core::protocol::{TypedAdoptionTarget, TypedDeclarationKind};
use provenance_core::{DeclarationAddress, ScopeId, StableId};

use super::identity::{requirement_address, source_address};
use super::reconcile::{
    desired_requirement, desired_rule, desired_source, reconciled_requirement, reconciled_rule,
    reconciled_source, requirement_changes, rule_changes, source_changes,
};
use super::{rule_address, CurrentTypedState, DesiredTypedIds};
use crate::state_store::{
    ReconcileState, ReconciledResource, TypedFieldChange, TypedResourceKind, TypedSpecInput,
};
use implementation::{current_value as current_implementation, matches as implementation_matches};
use relationships::DesiredRelationships;

pub(super) struct OwnershipDecision {
    conflicts: Vec<ReconciledResource>,
    refusal: Option<String>,
}

impl OwnershipDecision {
    pub(super) fn conflicts(&self) -> &[ReconciledResource] {
        &self.conflicts
    }

    pub(super) fn into_conflicts(self) -> Vec<ReconciledResource> {
        self.conflicts
    }

    pub(super) fn refuse(self) -> anyhow::Result<()> {
        anyhow::bail!(self
            .refusal
            .unwrap_or_else(|| { "typed declaration ownership conflict".to_string() }))
    }
}

pub(super) fn decide(
    scope_id: &ScopeId,
    input: &TypedSpecInput,
    current: &CurrentTypedState,
    ids: &DesiredTypedIds,
) -> anyhow::Result<OwnershipDecision> {
    let adopted = validate_targets(input, current)?;
    let relationships = DesiredRelationships::new(input, ids)?;
    let mut decision = OwnershipDecision {
        conflicts: Vec::new(),
        refusal: None,
    };

    decide_sources(
        scope_id,
        input,
        current,
        ids,
        &adopted,
        &relationships,
        &mut decision,
    )?;
    decide_requirements(
        scope_id,
        input,
        current,
        ids,
        &adopted,
        &relationships,
        &mut decision,
    )?;
    decide_rules(
        scope_id,
        input,
        current,
        ids,
        &adopted,
        &relationships,
        &mut decision,
    )?;
    Ok(decision)
}

#[allow(clippy::too_many_arguments)]
fn decide_sources(
    scope_id: &ScopeId,
    input: &TypedSpecInput,
    current: &CurrentTypedState,
    ids: &DesiredTypedIds,
    adopted: &BTreeSet<TypedAdoptionTarget>,
    relationships: &DesiredRelationships,
    decision: &mut OwnershipDecision,
) -> anyhow::Result<()> {
    for declaration in &input.sources {
        let id = &ids.sources[&declaration.key];
        let Some(existing) = current.sources.iter().find(|record| &record.id == id) else {
            continue;
        };
        let address = source_address(&input.spec, &declaration.key)?;
        let target = target(TypedDeclarationKind::Source, id);
        let desired = desired_source(
            scope_id,
            &input.declared_by,
            &address,
            id,
            declaration,
            &ids.sources,
        )?;
        let reconciled = reconciled_source(existing, desired, declaration, &ids.sources);
        let exact = same_source_definition(existing, &reconciled)
            && relationships.source_matches(id, current.relationships());
        let requested = adopted.contains(&target);
        if rejects(
            existing.declared_by.as_deref(),
            &input.declared_by,
            requested,
            exact,
        ) {
            let mut changes = source_changes(existing, &reconciled);
            relationships.add_source_change(id, current.relationships(), &mut changes);
            ensure_definition_change(existing, &reconciled, &mut changes);
            preserve_default_conflict(
                requested,
                existing.declared_by.as_deref(),
                &input.declared_by,
                &mut changes,
            );
            decision.reject(
                TypedResourceKind::Source,
                &declaration.key,
                None,
                address,
                id,
                existing.declared_by.as_deref(),
                &input.declared_by,
                requested,
                changes,
            );
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn decide_requirements(
    scope_id: &ScopeId,
    input: &TypedSpecInput,
    current: &CurrentTypedState,
    ids: &DesiredTypedIds,
    adopted: &BTreeSet<TypedAdoptionTarget>,
    relationships: &DesiredRelationships,
    decision: &mut OwnershipDecision,
) -> anyhow::Result<()> {
    for declaration in &input.requirements {
        let id = &ids.requirements[&declaration.key];
        let Some(existing) = current.requirements.iter().find(|record| &record.id == id) else {
            continue;
        };
        let address = requirement_address(&input.spec, &declaration.key)?;
        let target = target(TypedDeclarationKind::Requirement, id);
        let desired = desired_requirement(
            scope_id,
            &input.declared_by,
            &address,
            id,
            declaration,
            &ids.sources,
            &ids.requirements,
        )?;
        let reconciled = reconciled_requirement(existing, desired, declaration, &ids.requirements)?;
        let exact = same_requirement_definition(existing, &reconciled)
            && relationships.requirement_matches(id, current.relationships());
        let requested = adopted.contains(&target);
        if rejects(
            existing.declared_by.as_deref(),
            &input.declared_by,
            requested,
            exact,
        ) {
            let mut changes = requirement_changes(existing, &reconciled);
            relationships.add_requirement_change(id, current.relationships(), &mut changes);
            ensure_definition_change(existing, &reconciled, &mut changes);
            preserve_default_conflict(
                requested,
                existing.declared_by.as_deref(),
                &input.declared_by,
                &mut changes,
            );
            decision.reject(
                TypedResourceKind::Requirement,
                &declaration.key,
                None,
                address,
                id,
                existing.declared_by.as_deref(),
                &input.declared_by,
                requested,
                changes,
            );
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn decide_rules(
    scope_id: &ScopeId,
    input: &TypedSpecInput,
    current: &CurrentTypedState,
    ids: &DesiredTypedIds,
    adopted: &BTreeSet<TypedAdoptionTarget>,
    relationships: &DesiredRelationships,
    decision: &mut OwnershipDecision,
) -> anyhow::Result<()> {
    for declaration in &input.rules {
        let address = rule_address(&input.spec, declaration)?;
        let id = &ids.rules[&address];
        let Some(existing) = current.rules.iter().find(|record| &record.id == id) else {
            continue;
        };
        let target = target(TypedDeclarationKind::Rule, id);
        let desired = desired_rule(
            scope_id,
            &input.declared_by,
            &address,
            id,
            declaration,
            &ids.requirements,
        )?;
        let reconciled = reconciled_rule(existing, desired, declaration, &ids.requirements)?;
        let implementation_exact = implementation_matches(
            id,
            declaration.implementation.as_ref(),
            &current.implementation_bindings,
            &input.declared_by,
        );
        let exact = same_rule_definition(existing, &reconciled)
            && implementation_exact
            && relationships.rule_matches(id, current.relationships());
        let requested = adopted.contains(&target);
        if rejects(
            existing.declared_by.as_deref(),
            &input.declared_by,
            requested,
            exact,
        ) {
            let mut changes = rule_changes(existing, &reconciled);
            if let Some(implementation) = declaration
                .implementation
                .as_ref()
                .filter(|_| !implementation_exact)
            {
                changes.push(TypedFieldChange {
                    field: "implementation".to_string(),
                    before: current_implementation(id, &current.implementation_bindings),
                    after: serde_json::to_value(implementation)?,
                });
            }
            relationships.add_rule_change(id, current.relationships(), &mut changes);
            ensure_definition_change(existing, &reconciled, &mut changes);
            preserve_default_conflict(
                requested,
                existing.declared_by.as_deref(),
                &input.declared_by,
                &mut changes,
            );
            decision.reject(
                TypedResourceKind::Rule,
                &declaration.key,
                super::rule_addresses::local_parent(&address),
                address,
                id,
                existing.declared_by.as_deref(),
                &input.declared_by,
                requested,
                changes,
            );
        }
    }
    Ok(())
}

impl OwnershipDecision {
    #[allow(clippy::too_many_arguments)]
    fn reject(
        &mut self,
        kind: TypedResourceKind,
        key: &str,
        parent: Option<String>,
        address: DeclarationAddress,
        id: &StableId,
        current_owner: Option<&str>,
        desired_owner: &str,
        adoption_requested: bool,
        changes: Vec<TypedFieldChange>,
    ) {
        if self.refusal.is_none() {
            self.refusal = Some(if adoption_requested {
                format!(
                    "adoption target `{}` conflicts with its canonical declaration",
                    id.as_str()
                )
            } else {
                format!(
                    "record `{}` is not owned by `{desired_owner}` (declared_by: {})",
                    id.as_str(),
                    current_owner.unwrap_or("unowned")
                )
            });
        }
        self.conflicts.push(ReconciledResource {
            kind,
            key: key.to_string(),
            parent,
            address,
            id: id.clone(),
            state: ReconcileState::Conflict,
            changes,
        });
    }
}

fn rejects(owner: Option<&str>, desired: &str, adopted: bool, exact: bool) -> bool {
    match owner {
        Some(owner) if owner == desired => adopted && !exact,
        Some(_) => true,
        None => !adopted || !exact,
    }
}

fn preserve_default_conflict(
    adoption_requested: bool,
    current_owner: Option<&str>,
    desired_owner: &str,
    changes: &mut Vec<TypedFieldChange>,
) {
    let foreign_owner = current_owner.is_some_and(|owner| owner != desired_owner);
    if !adoption_requested || foreign_owner {
        *changes = vec![owner_change(current_owner, desired_owner)];
    }
}

fn target(kind: TypedDeclarationKind, id: &StableId) -> TypedAdoptionTarget {
    TypedAdoptionTarget {
        kind,
        id: id.as_str().to_string(),
    }
}

fn validate_targets(
    input: &TypedSpecInput,
    current: &CurrentTypedState,
) -> anyhow::Result<BTreeSet<TypedAdoptionTarget>> {
    let mut targets = BTreeSet::new();
    for target in &input.adopt_unowned {
        StableId::new(&target.id).map_err(|_| {
            anyhow::anyhow!(
                "adoption target id `{}` must use lowercase ASCII letters, digits, '_' or '-'",
                target.id
            )
        })?;
        anyhow::ensure!(
            targets.insert(target.clone()),
            "duplicate adoption target `{}:{}`",
            kind_name(target.kind),
            target.id
        );
    }
    for target in &input.adopt_unowned {
        let (declarations, exact) = match target.kind {
            TypedDeclarationKind::Source => (
                input.sources.len(),
                input
                    .sources
                    .iter()
                    .filter(|value| value.id.as_deref() == Some(&target.id))
                    .count(),
            ),
            TypedDeclarationKind::Requirement => (
                input.requirements.len(),
                input
                    .requirements
                    .iter()
                    .filter(|value| value.id.as_deref() == Some(&target.id))
                    .count(),
            ),
            TypedDeclarationKind::Rule => (
                input.rules.len(),
                input
                    .rules
                    .iter()
                    .filter(|value| value.id.as_deref() == Some(&target.id))
                    .count(),
            ),
        };
        anyhow::ensure!(
            declarations > 0,
            "adoption target `{}:{}` does not name a declaration in this document",
            kind_name(target.kind),
            target.id
        );
        anyhow::ensure!(
            exact == 1,
            "adoption target `{}:{}` must name exactly one declaration with the same explicit id",
            kind_name(target.kind),
            target.id
        );
    }
    for target in &input.adopt_unowned {
        let exists = match target.kind {
            TypedDeclarationKind::Source => current
                .sources
                .iter()
                .any(|value| value.id.as_str() == target.id),
            TypedDeclarationKind::Requirement => current
                .requirements
                .iter()
                .any(|value| value.id.as_str() == target.id),
            TypedDeclarationKind::Rule => current
                .rules
                .iter()
                .any(|value| value.id.as_str() == target.id),
        };
        anyhow::ensure!(
            exists,
            "adoption target `{}:{}` does not exist in canonical state",
            kind_name(target.kind),
            target.id
        );
    }
    Ok(targets)
}

const fn kind_name(kind: TypedDeclarationKind) -> &'static str {
    match kind {
        TypedDeclarationKind::Source => "source",
        TypedDeclarationKind::Requirement => "requirement",
        TypedDeclarationKind::Rule => "rule",
    }
}

fn same_source_definition(
    current: &provenance_core::Source,
    desired: &provenance_core::Source,
) -> bool {
    let mut normalized = current.clone();
    normalized.declared_by.clone_from(&desired.declared_by);
    normalized
        .declaration_address
        .clone_from(&desired.declaration_address);
    normalized == *desired
}

fn same_requirement_definition(
    current: &provenance_core::Requirement,
    desired: &provenance_core::Requirement,
) -> bool {
    let mut normalized = current.clone();
    normalized.declared_by.clone_from(&desired.declared_by);
    normalized
        .declaration_address
        .clone_from(&desired.declaration_address);
    normalized == *desired
}

fn same_rule_definition(current: &provenance_core::Rule, desired: &provenance_core::Rule) -> bool {
    let mut normalized = current.clone();
    normalized.declared_by.clone_from(&desired.declared_by);
    normalized
        .declaration_address
        .clone_from(&desired.declaration_address);
    normalized == *desired
}

fn ensure_definition_change<T: serde::Serialize + PartialEq>(
    current: &T,
    desired: &T,
    changes: &mut Vec<TypedFieldChange>,
) {
    if current != desired && changes.len() <= 2 {
        changes.push(TypedFieldChange {
            field: "definition".to_string(),
            before: serde_json::to_value(current).expect("canonical record serializes"),
            after: serde_json::to_value(desired).expect("canonical record serializes"),
        });
    }
}

fn owner_change(current: Option<&str>, desired: &str) -> TypedFieldChange {
    TypedFieldChange {
        field: "declared_by".to_string(),
        before: current.unwrap_or("unowned").into(),
        after: desired.into(),
    }
}
