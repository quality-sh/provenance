use std::collections::BTreeMap;

use provenance_core::{DeclarationAddress, Requirement, Rule, Source, StableId};

use super::rule_addresses::local_parent;
use crate::state_store::{ReconcileState, ReconciledResource, TypedResourceKind};

pub(super) fn delete_omitted_sources(
    records: &mut Vec<Source>,
    resources: &mut Vec<ReconciledResource>,
    spec: &str,
    owner: &str,
    desired: &BTreeMap<String, StableId>,
) {
    records.retain(|record| {
        let omitted = owned_by_spec(
            record.declared_by.as_deref(),
            record.declaration_address.as_ref(),
            spec,
            owner,
        ) && !desired.values().any(|id| id == &record.id);
        if !omitted {
            return true;
        }
        let address = typed_address(record.declaration_address.as_ref());
        resources.push(deleted_resource(
            TypedResourceKind::Source,
            declaration_key(&address),
            None,
            address,
            record.id.clone(),
        ));
        false
    });
}

pub(super) fn delete_omitted_requirements(
    records: &mut Vec<Requirement>,
    resources: &mut Vec<ReconciledResource>,
    spec: &str,
    owner: &str,
    desired: &BTreeMap<String, StableId>,
) {
    records.retain(|record| {
        let omitted = owned_by_spec(
            record.declared_by.as_deref(),
            record.declaration_address.as_ref(),
            spec,
            owner,
        ) && !desired.values().any(|id| id == &record.id);
        if !omitted {
            return true;
        }
        let address = typed_address(record.declaration_address.as_ref());
        resources.push(deleted_resource(
            TypedResourceKind::Requirement,
            declaration_key(&address),
            None,
            address,
            record.id.clone(),
        ));
        false
    });
}

pub(super) fn delete_omitted_rules(
    records: &mut Vec<Rule>,
    resources: &mut Vec<ReconciledResource>,
    spec: &str,
    owner: &str,
    desired: &BTreeMap<DeclarationAddress, StableId>,
) {
    records.retain(|record| {
        let omitted = owned_by_spec(
            record.declared_by.as_deref(),
            record.declaration_address.as_ref(),
            spec,
            owner,
        ) && !desired.values().any(|id| id == &record.id);
        if !omitted {
            return true;
        }
        let address = typed_address(record.declaration_address.as_ref());
        let key = declaration_key(&address);
        resources.push(deleted_resource(
            TypedResourceKind::Rule,
            key,
            local_parent(&address),
            address,
            record.id.clone(),
        ));
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

const fn deleted_resource(
    kind: TypedResourceKind,
    key: String,
    parent: Option<String>,
    address: DeclarationAddress,
    id: StableId,
) -> ReconciledResource {
    ReconciledResource {
        kind,
        key,
        parent,
        address,
        id,
        state: ReconcileState::Deleted,
        changes: Vec::new(),
    }
}
