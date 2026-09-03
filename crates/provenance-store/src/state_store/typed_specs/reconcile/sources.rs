use std::collections::BTreeMap;

use provenance_core::{
    DeclarationAddress, ScopeId, Source, SourceType, StableId, SUPPORTED_SCHEMA_VERSION,
};

use super::super::super::{
    ReconcileState, ReconciledResource, TypedFieldChange, TypedResourceKind, TypedSourceInput,
};
use super::super::identity::source_address;
use super::super::lifecycle::retire_omitted_sources;
use super::changes::{changed, resource, state_after_change};
use super::references;

pub(in crate::state_store::typed_specs) fn reconcile_sources(
    mut records: Vec<Source>,
    spec: &str,
    scope_id: &ScopeId,
    owner: &str,
    declarations: Vec<TypedSourceInput>,
    ids: &BTreeMap<String, StableId>,
) -> anyhow::Result<(Vec<Source>, Vec<ReconciledResource>)> {
    let mut resources = Vec::new();
    for declaration in declarations {
        let id = ids[&declaration.key].clone();
        let address = source_address(spec, &declaration.key)?;
        let desired = desired_source(scope_id, owner, &address, &id, &declaration, ids)?;
        let (state, changes) = upsert_source(&mut records, desired, &declaration, ids);
        resources.push(resource(
            TypedResourceKind::Source,
            declaration.key,
            None,
            address,
            id,
            state,
            changes,
        ));
    }
    retire_omitted_sources(&mut records, &mut resources, spec, owner, ids);
    records.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    Ok((records, resources))
}

pub(in crate::state_store::typed_specs) fn desired_source(
    scope_id: &ScopeId,
    owner: &str,
    address: &DeclarationAddress,
    id: &StableId,
    declaration: &TypedSourceInput,
    source_ids: &BTreeMap<String, StableId>,
) -> anyhow::Result<Source> {
    let mut source = Source {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope_id.clone(),
        id: id.clone(),
        declared_by: Some(owner.to_string()),
        declaration_address: Some(address.clone()),
        retired: false,
        name: declaration.name.clone(),
        source_type: source_type(&declaration.kind)?,
        url: declaration.url.clone(),
        reference: declaration.reference.clone(),
        commit_pin: None,
        effective_date: None,
        review_date: None,
        supersedes: Vec::new(),
        superseded_by: None,
        origin_thread: None,
        origin_message: None,
    };
    references::apply_source(&mut source, declaration, source_ids);
    Ok(source)
}

fn source_type(kind: &str) -> anyhow::Result<SourceType> {
    SourceType::parse(kind).or_else(|_| match kind.to_ascii_lowercase().as_str() {
        "linear" | "github" | "jira" => Ok(SourceType::ExternalIntegration),
        _ => anyhow::bail!("source kind `{kind}` is not supported"),
    })
}

fn upsert_source(
    records: &mut Vec<Source>,
    desired: Source,
    declaration: &TypedSourceInput,
    source_ids: &BTreeMap<String, StableId>,
) -> (ReconcileState, Vec<TypedFieldChange>) {
    let Some(existing) = records.iter_mut().find(|record| record.id == desired.id) else {
        records.push(desired);
        return (ReconcileState::Created, Vec::new());
    };
    let before = existing.clone();
    *existing = reconciled_source(&before, desired, declaration, source_ids);
    let changes = source_changes(&before, existing);
    (
        state_after_change(before.declaration_address.as_ref(), existing, &before),
        changes,
    )
}

/// The current record with the declaration's fields laid over it. A
/// reference field the declaration leaves out stays as it was.
pub(in crate::state_store::typed_specs) fn reconciled_source(
    current: &Source,
    desired: Source,
    declaration: &TypedSourceInput,
    source_ids: &BTreeMap<String, StableId>,
) -> Source {
    let mut reconciled = current.clone();
    reconciled.declared_by = desired.declared_by;
    reconciled.declaration_address = desired.declaration_address;
    reconciled.retired = false;
    reconciled.name = desired.name;
    reconciled.source_type = desired.source_type;
    if desired.url.is_some() {
        reconciled.url = desired.url;
    }
    if desired.reference.is_some() {
        reconciled.reference = desired.reference;
    }
    references::apply_source(&mut reconciled, declaration, source_ids);
    reconciled
}

pub(in crate::state_store::typed_specs) fn source_changes(
    before: &Source,
    after: &Source,
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
    changed(&mut changes, "name", &before.name, &after.name);
    changed(
        &mut changes,
        "kind",
        &before.source_type,
        &after.source_type,
    );
    changed(&mut changes, "url", &before.url, &after.url);
    changed(
        &mut changes,
        "reference",
        &before.reference,
        &after.reference,
    );
    changed(
        &mut changes,
        "supersedes",
        &before.supersedes,
        &after.supersedes,
    );
    changes
}
