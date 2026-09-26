//! The reconcile state of one resource and the field changes it carries.

use super::super::super::{
    ReconcileState, ReconciledResource, TypedFieldChange, TypedResourceKind,
};
use super::super::DesiredTypedIds;
use provenance_core::{DeclarationAddress, ScopeId, StableId};

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

pub(in crate::state_store::typed_specs) trait DeclarationRecord:
    Clone + PartialEq + Sized
{
    type Declaration;

    const KIND: TypedResourceKind;

    fn key(declaration: &Self::Declaration) -> &str;
    fn address(spec: &str, declaration: &Self::Declaration) -> anyhow::Result<DeclarationAddress>;
    fn parent(address: &DeclarationAddress) -> Option<String>;
    fn desired_id<'a>(
        address: &DeclarationAddress,
        declaration: &Self::Declaration,
        ids: &'a DesiredTypedIds,
    ) -> &'a StableId;
    fn desired_contains(ids: &DesiredTypedIds, id: &StableId) -> bool;
    fn desired(
        scope_id: &ScopeId,
        owner: &str,
        address: &DeclarationAddress,
        id: &StableId,
        declaration: &Self::Declaration,
        ids: &DesiredTypedIds,
    ) -> anyhow::Result<Self>;
    fn reconciled(
        &self,
        desired: Self,
        declaration: &Self::Declaration,
        ids: &DesiredTypedIds,
    ) -> anyhow::Result<Self>;
    fn changes(&self, after: &Self) -> Vec<TypedFieldChange>;

    fn same_definition(&self, desired: &Self) -> bool {
        let mut normalized = self.clone();
        normalized.copy_declaration_identity(desired);
        normalized == *desired
    }

    fn stable_id(&self) -> &StableId;
    fn declared_by(&self) -> Option<&str>;
    fn declaration_address(&self) -> Option<&DeclarationAddress>;
    fn copy_declaration_identity(&mut self, desired: &Self);
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
            requirement: None,
            requirements: vec!["access".to_string()],
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
