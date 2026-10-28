//! Reconciles typed declarations with canonical records.

pub(super) mod changes;
mod references;
mod requirements;
mod rules;
mod sources;

use provenance_core::ScopeId;

use self::changes::{resource, state_after_change, DeclarationRecord};
use super::deletion::delete_omitted;
use super::{DesiredTypedIds, ReconciledResource};

pub(super) use references::{ensure_acyclic, ensure_resolutions_exist};

pub(super) fn reconcile<T: DeclarationRecord>(
    mut records: Vec<T>,
    spec: &str,
    scope_id: &ScopeId,
    owner: &str,
    declarations: Vec<T::Declaration>,
    ids: &DesiredTypedIds,
) -> anyhow::Result<(Vec<T>, Vec<ReconciledResource>)> {
    let mut resources = Vec::new();
    for declaration in declarations {
        let address = T::address(spec, &declaration)?;
        let id = T::desired_id(&address, &declaration, ids).clone();
        let desired = T::desired(scope_id, owner, &address, &id, &declaration, ids)?;
        let (state, changes) = upsert(&mut records, desired, &declaration, ids)?;
        resources.push(resource(
            T::KIND,
            T::key(&declaration).to_string(),
            T::parent(&address),
            address,
            id,
            state,
            changes,
        ));
    }
    delete_omitted::<T>(&mut records, &mut resources, spec, owner, ids);
    records.sort_by(|left, right| left.stable_id().as_str().cmp(right.stable_id().as_str()));
    Ok((records, resources))
}

fn upsert<T: DeclarationRecord>(
    records: &mut Vec<T>,
    desired: T,
    declaration: &T::Declaration,
    ids: &DesiredTypedIds,
) -> anyhow::Result<(super::ReconcileState, Vec<super::TypedFieldChange>)> {
    let Some(existing) = records
        .iter_mut()
        .find(|record| record.stable_id() == desired.stable_id())
    else {
        records.push(desired);
        return Ok((super::ReconcileState::Created, Vec::new()));
    };
    let before = existing.clone();
    *existing = before.reconciled(desired, declaration, ids)?;
    let changes = before.changes(existing);
    Ok((
        state_after_change(before.declaration_address(), existing, &before),
        changes,
    ))
}
