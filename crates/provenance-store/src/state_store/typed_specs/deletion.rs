use provenance_core::DeclarationAddress;

use super::reconcile::changes::DeclarationRecord;
use super::{DesiredTypedIds, ReconcileState, ReconciledResource};

pub(super) fn delete_omitted<T: DeclarationRecord>(
    records: &mut Vec<T>,
    resources: &mut Vec<ReconciledResource>,
    spec: &str,
    owner: &str,
    desired: &DesiredTypedIds,
) {
    records.retain(|record| {
        let omitted = owned_by_spec(
            record.declared_by(),
            record.declaration_address(),
            spec,
            owner,
        ) && !T::desired_contains(desired, record.stable_id());
        if !omitted {
            return true;
        }
        let address = typed_address(record.declaration_address());
        resources.push(ReconciledResource {
            kind: T::KIND,
            key: declaration_key(&address),
            parent: T::parent(&address),
            address,
            id: record.stable_id().clone(),
            state: ReconcileState::Deleted,
            changes: Vec::new(),
        });
        false
    });
}

fn owned_by_spec(
    declared_by: Option<&str>,
    address: Option<&DeclarationAddress>,
    spec: &str,
    owner: &str,
) -> bool {
    declared_by == Some(owner)
        && address
            .is_some_and(|address| address.segments().first().is_some_and(|part| part == spec))
}

fn typed_address(address: Option<&DeclarationAddress>) -> DeclarationAddress {
    address
        .cloned()
        .expect("owned typed declaration has an address")
}

fn declaration_key(address: &DeclarationAddress) -> String {
    address
        .segments()
        .last()
        .expect("declaration address is non-empty")
        .clone()
}
