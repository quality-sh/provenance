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

#[cfg(test)]
mod tests {
    use super::DeclarationRecord;
    use crate::state_store::{
        TypedRequirementInput, TypedResourceKind, TypedRuleInput, TypedSourceInput,
    };
    use provenance_core::{Requirement, Rule, Source};

    fn declaration_shape<T: DeclarationRecord>(
        spec: &str,
        declaration: &T::Declaration,
    ) -> (TypedResourceKind, Vec<String>, Option<String>) {
        let address = T::address(spec, declaration).unwrap();
        (T::KIND, address.segments().to_vec(), T::parent(&address))
    }

    #[test]
    fn declaration_records_define_their_resource_shape() {
        let source = TypedSourceInput {
            key: "policy".to_string(),
            id: None,
            name: "Policy".to_string(),
            kind: "document".to_string(),
            url: None,
            reference: None,
            supersedes: None,
        };
        let requirement = TypedRequirementInput {
            key: "access".to_string(),
            id: None,
            statement: "The system controls access".to_string(),
            description: None,
            sources: Vec::new(),
            refines: None,
            depends_on: None,
            supersedes: None,
            spawned_by: None,
        };
        let rule = TypedRuleInput {
            key: "deny".to_string(),
            id: None,
            address: None,
            requirement: Some("access".to_string()),
            requirements: Vec::new(),
            statement: "The system denies unknown callers".to_string(),
            name: None,
            description: None,
            resolution_ids: None,
            implementation: None,
        };

        assert_eq!(
            declaration_shape::<Source>("security", &source),
            (
                TypedResourceKind::Source,
                vec![
                    "security".to_string(),
                    "source".to_string(),
                    "policy".to_string()
                ],
                None,
            )
        );
        assert_eq!(
            declaration_shape::<Requirement>("security", &requirement),
            (
                TypedResourceKind::Requirement,
                vec![
                    "security".to_string(),
                    "requirement".to_string(),
                    "access".to_string(),
                ],
                None,
            )
        );
        assert_eq!(
            declaration_shape::<Rule>("security", &rule),
            (
                TypedResourceKind::Rule,
                vec![
                    "security".to_string(),
                    "requirement".to_string(),
                    "access".to_string(),
                    "rule".to_string(),
                    "deny".to_string(),
                ],
                Some("access".to_string()),
            )
        );
    }
}
