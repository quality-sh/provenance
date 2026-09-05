//! The reconcile state of one resource and the field changes it carries.

use super::super::super::{
    ReconcileState, ReconciledResource, TypedFieldChange, TypedResourceKind,
};
use provenance_core::{DeclarationAddress, Requirement, Rule, Source, StableId};

pub(super) fn changed<T: PartialEq + serde::Serialize>(
    changes: &mut Vec<TypedFieldChange>,
    field: &str,
    before: &T,
    after: &T,
) {
    if before != after {
        changes.push(TypedFieldChange {
            field: field.to_string(),
            before: serde_json::to_value(before).expect("canonical field serializes"),
            after: serde_json::to_value(after).expect("canonical field serializes"),
        });
    }
}

pub(super) fn state_after_change<T: PartialEq + DeclarationRecord>(
    previous_address: Option<&DeclarationAddress>,
    changed: &T,
    before: &T,
) -> ReconcileState {
    if changed == before {
        ReconcileState::Unchanged
    } else if previous_address != changed.declaration_address() {
        ReconcileState::Moved
    } else {
        ReconcileState::Updated
    }
}

pub(super) trait DeclarationRecord {
    fn declaration_address(&self) -> Option<&DeclarationAddress>;
}

impl DeclarationRecord for Source {
    fn declaration_address(&self) -> Option<&DeclarationAddress> {
        self.declaration_address.as_ref()
    }
}

impl DeclarationRecord for Requirement {
    fn declaration_address(&self) -> Option<&DeclarationAddress> {
        self.declaration_address.as_ref()
    }
}

impl DeclarationRecord for Rule {
    fn declaration_address(&self) -> Option<&DeclarationAddress> {
        self.declaration_address.as_ref()
    }
}

pub(super) const fn resource(
    kind: TypedResourceKind,
    key: String,
    parent: Option<String>,
    address: DeclarationAddress,
    id: StableId,
    state: ReconcileState,
    changes: Vec<TypedFieldChange>,
) -> ReconciledResource {
    ReconciledResource {
        kind,
        key,
        parent,
        address,
        id,
        state,
        changes,
    }
}
