//! Exact, per-target authorization for one-time declaration adoption.

mod implementation;
mod targets;
use targets::validate_targets;
mod relationships;

use std::collections::BTreeSet;

use provenance_core::protocol::{TypedAdoptionTarget, TypedDeclarationKind};
use provenance_core::{DeclarationAddress, Requirement, Rule, ScopeId, Source, StableId};

use super::reconcile::changes::DeclarationRecord;
use super::{CurrentTypedState, DesiredTypedIds};
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
        Err(crate::write_error::SourceFailure::wrap(
            crate::write_error::WriteFailure::OwnershipConflict {
                conflicts: self.conflicts,
            },
            anyhow::anyhow!(self
                .refusal
                .unwrap_or_else(|| "typed declaration ownership conflict".to_string())),
        ))
    }
}

pub(super) fn decide(
    scope_id: &ScopeId,
    input: &TypedSpecInput,
    current: &CurrentTypedState,
    ids: &DesiredTypedIds,
) -> anyhow::Result<OwnershipDecision> {
    let adopted = validate_targets(input, current).map_err(|error| {
        crate::write_error::SourceFailure::wrap(
            crate::write_error::WriteFailure::InvalidDeclaration,
            error,
        )
    })?;
    let relationships = DesiredRelationships::new(input, ids)?;
    let mut decision = OwnershipDecision {
        conflicts: Vec::new(),
        refusal: None,
    };

    decide_declarations::<Source, _, _>(
        scope_id,
        input,
        &current.sources,
        &input.sources,
        ids,
        &adopted,
        &mut decision,
        |id, _| relationships.source_matches(id, current.relationships()),
        |id, _, changes| {
            relationships.add_source_change(id, current.relationships(), changes);
            Ok(())
        },
    )?;
    decide_declarations::<Requirement, _, _>(
        scope_id,
        input,
        &current.requirements,
        &input.requirements,
        ids,
        &adopted,
        &mut decision,
        |id, _| relationships.requirement_matches(id, current.relationships()),
        |id, _, changes| {
            relationships.add_requirement_change(id, current.relationships(), changes);
            Ok(())
        },
    )?;
    decide_declarations::<Rule, _, _>(
        scope_id,
        input,
        &current.rules,
        &input.rules,
        ids,
        &adopted,
        &mut decision,
        |id, declaration| {
            implementation_matches(
                id,
                declaration.implementation.as_ref(),
                &current.implementation_bindings,
                &input.declared_by,
            ) && relationships.rule_matches(id, current.relationships())
        },
        |id, declaration, changes| {
            if let Some(implementation) = declaration.implementation.as_ref().filter(|_| {
                !implementation_matches(
                    id,
                    declaration.implementation.as_ref(),
                    &current.implementation_bindings,
                    &input.declared_by,
                )
            }) {
                changes.push(TypedFieldChange {
                    field: "implementation".to_string(),
                    before: current_implementation(id, &current.implementation_bindings),
                    after: serde_json::to_value(implementation)?,
                });
            }
            relationships.add_rule_change(id, current.relationships(), changes);
            Ok(())
        },
    )?;
    Ok(decision)
}

#[allow(clippy::too_many_arguments)]
fn decide_declarations<T, Exact, AddChanges>(
    scope_id: &ScopeId,
    input: &TypedSpecInput,
    records: &[T],
    declarations: &[T::Declaration],
    ids: &DesiredTypedIds,
    adopted: &BTreeSet<TypedAdoptionTarget>,
    decision: &mut OwnershipDecision,
    extra_exact: Exact,
    add_changes: AddChanges,
) -> anyhow::Result<()>
where
    T: DeclarationRecord + serde::Serialize,
    Exact: Fn(&StableId, &T::Declaration) -> bool,
    AddChanges: Fn(&StableId, &T::Declaration, &mut Vec<TypedFieldChange>) -> anyhow::Result<()>,
{
    for declaration in declarations {
        let address = T::address(&input.spec, declaration)?;
        let id = T::desired_id(&address, declaration, ids);
        let Some(existing) = records.iter().find(|record| record.stable_id() == id) else {
            continue;
        };
        let target = target(T::KIND, id);
        let desired = T::desired(scope_id, &input.declared_by, &address, id, declaration, ids)?;
        let reconciled = existing.reconciled(desired, declaration, ids)?;
        let exact = existing.same_definition(&reconciled) && extra_exact(id, declaration);
        let requested = adopted.contains(&target);
        if rejects(existing.declared_by(), &input.declared_by, requested, exact) {
            let mut changes = existing.changes(&reconciled);
            add_changes(id, declaration, &mut changes)?;
            ensure_definition_change(existing, &reconciled, &mut changes);
            preserve_default_conflict(
                requested,
                existing.declared_by(),
                &input.declared_by,
                &mut changes,
            );
            decision.reject(
                T::KIND,
                T::key(declaration),
                T::parent(&address),
                address,
                id,
                existing.declared_by(),
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

fn target(kind: TypedResourceKind, id: &StableId) -> TypedAdoptionTarget {
    TypedAdoptionTarget {
        kind: match kind {
            TypedResourceKind::Source => TypedDeclarationKind::Source,
            TypedResourceKind::Requirement => TypedDeclarationKind::Requirement,
            TypedResourceKind::Rule => TypedDeclarationKind::Rule,
        },
        id: id.as_str().to_string(),
    }
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
