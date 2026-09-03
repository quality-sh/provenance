use std::collections::BTreeMap;

use provenance_core::{
    DeclarationAddress, Requirement, RequirementStatus, ScopeId, SourceReference, StableId,
    SUPPORTED_SCHEMA_VERSION,
};

use super::super::super::{
    ReconcileState, ReconciledResource, TypedFieldChange, TypedRequirementInput, TypedResourceKind,
};
use super::super::identity::requirement_address;
use super::super::lifecycle::retire_omitted_requirements;
use super::changes::{changed, resource, state_after_change};
use super::references;

pub(in crate::state_store::typed_specs) fn reconcile_requirements(
    mut records: Vec<Requirement>,
    spec: &str,
    scope_id: &ScopeId,
    owner: &str,
    declarations: Vec<TypedRequirementInput>,
    ids: &BTreeMap<String, StableId>,
    source_ids: &BTreeMap<String, StableId>,
) -> anyhow::Result<(Vec<Requirement>, Vec<ReconciledResource>)> {
    let mut resources = Vec::new();
    for declaration in declarations {
        let id = ids[&declaration.key].clone();
        let address = requirement_address(spec, &declaration.key)?;
        let desired = desired_requirement(
            scope_id,
            owner,
            &address,
            &id,
            &declaration,
            source_ids,
            ids,
        )?;
        let (state, changes) = upsert_requirement(&mut records, desired, &declaration, ids)?;
        resources.push(resource(
            TypedResourceKind::Requirement,
            declaration.key,
            None,
            address,
            id,
            state,
            changes,
        ));
    }
    retire_omitted_requirements(&mut records, &mut resources, spec, owner, ids);
    records.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    Ok((records, resources))
}

pub(in crate::state_store::typed_specs) fn desired_requirement(
    scope_id: &ScopeId,
    owner: &str,
    address: &DeclarationAddress,
    id: &StableId,
    declaration: &TypedRequirementInput,
    source_ids: &BTreeMap<String, StableId>,
    requirement_ids: &BTreeMap<String, StableId>,
) -> anyhow::Result<Requirement> {
    let source_refs = declaration
        .sources
        .iter()
        .map(|key| SourceReference {
            source_id: source_ids[key].clone(),
            clause: None,
        })
        .collect();
    let mut requirement = Requirement {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope_id.clone(),
        id: id.clone(),
        declared_by: Some(owner.to_string()),
        declaration_address: Some(address.clone()),
        retired: false,
        statement: declaration.statement.clone(),
        description: declaration.description.clone(),
        fog: None,
        status: RequirementStatus::Active,
        domain_id: None,
        source_refs,
        refines: None,
        depends_on: Vec::new(),
        supersedes: Vec::new(),
        spawned_by: None,
        origin_thread: None,
        origin_message: None,
    };
    references::apply_requirement(&mut requirement, declaration, requirement_ids)?;
    Ok(requirement)
}

fn upsert_requirement(
    records: &mut Vec<Requirement>,
    desired: Requirement,
    declaration: &TypedRequirementInput,
    requirement_ids: &BTreeMap<String, StableId>,
) -> anyhow::Result<(ReconcileState, Vec<TypedFieldChange>)> {
    let Some(existing) = records.iter_mut().find(|record| record.id == desired.id) else {
        records.push(desired);
        return Ok((ReconcileState::Created, Vec::new()));
    };
    let before = existing.clone();
    *existing = reconciled_requirement(&before, desired, declaration, requirement_ids)?;
    let changes = requirement_changes(&before, existing);
    Ok((
        state_after_change(before.declaration_address.as_ref(), existing, &before),
        changes,
    ))
}

/// The current record with the declaration's fields laid over it. Citations
/// append; a reference field the declaration leaves out stays as it was.
pub(in crate::state_store::typed_specs) fn reconciled_requirement(
    current: &Requirement,
    desired: Requirement,
    declaration: &TypedRequirementInput,
    requirement_ids: &BTreeMap<String, StableId>,
) -> anyhow::Result<Requirement> {
    let mut reconciled = current.clone();
    reconciled.declared_by = desired.declared_by;
    reconciled.declaration_address = desired.declaration_address;
    reconciled.retired = false;
    reconciled.statement = desired.statement;
    if desired.description.is_some() {
        reconciled.description = desired.description;
    }
    for source in desired.source_refs {
        if !reconciled
            .source_refs
            .iter()
            .any(|existing| existing.source_id == source.source_id)
        {
            reconciled.source_refs.push(source);
        }
    }
    reconciled.source_refs.sort_by(|left, right| {
        left.source_id
            .as_str()
            .cmp(right.source_id.as_str())
            .then(left.clause.cmp(&right.clause))
    });
    references::apply_requirement(&mut reconciled, declaration, requirement_ids)?;
    Ok(reconciled)
}

pub(in crate::state_store::typed_specs) fn requirement_changes(
    before: &Requirement,
    after: &Requirement,
) -> Vec<TypedFieldChange> {
    let mut changes = Vec::new();
    changed(
        &mut changes,
        "declared_by",
        &before.declared_by,
        &after.declared_by,
    );
    changed(
        &mut changes,
        "address",
        &before.declaration_address,
        &after.declaration_address,
    );
    changed(&mut changes, "retired", &before.retired, &after.retired);
    changed(
        &mut changes,
        "statement",
        &before.statement,
        &after.statement,
    );
    changed(
        &mut changes,
        "description",
        &before.description,
        &after.description,
    );
    changed(
        &mut changes,
        "sources",
        &before.source_refs,
        &after.source_refs,
    );
    changed(&mut changes, "refines", &before.refines, &after.refines);
    changed(
        &mut changes,
        "depends_on",
        &before.depends_on,
        &after.depends_on,
    );
    changed(
        &mut changes,
        "supersedes",
        &before.supersedes,
        &after.supersedes,
    );
    changed(
        &mut changes,
        "spawned_by",
        &before.spawned_by,
        &after.spawned_by,
    );
    changes
}
