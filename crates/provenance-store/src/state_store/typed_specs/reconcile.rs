use std::collections::BTreeMap;

use provenance_core::{
    Requirement, RequirementStatus, Rule, RuleSeverity, RuleStatus, ScopeId, Source,
    SourceReference, SourceType, StableId, SUPPORTED_SCHEMA_VERSION,
};

use super::super::{
    ReconcileState, ReconciledResource, TypedFieldChange, TypedRequirementInput, TypedResourceKind,
    TypedRuleInput, TypedSourceInput,
};
use super::identity::{requirement_address, source_address};
use super::lifecycle::{retire_omitted_requirements, retire_omitted_rules, retire_omitted_sources};
use super::rule_addresses::{local_parent, rule_address};

pub(super) fn reconcile_sources(
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
        let desired = desired_source(scope_id, owner, &address, &id, &declaration)?;
        let (state, changes) = upsert_source(&mut records, desired);
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

pub(super) fn desired_source(
    scope_id: &ScopeId,
    owner: &str,
    address: &provenance_core::DeclarationAddress,
    id: &StableId,
    declaration: &TypedSourceInput,
) -> anyhow::Result<Source> {
    Ok(Source {
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
        superseded_by: None,
        origin_thread: None,
        origin_message: None,
    })
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
) -> (ReconcileState, Vec<TypedFieldChange>) {
    let Some(existing) = records.iter_mut().find(|record| record.id == desired.id) else {
        records.push(desired);
        return (ReconcileState::Created, Vec::new());
    };
    let before = existing.clone();
    *existing = reconciled_source(&before, desired);
    let changes = source_changes(&before, existing);
    (
        state_after_change(before.declaration_address.as_ref(), existing, &before),
        changes,
    )
}

pub(super) fn reconciled_source(current: &Source, desired: Source) -> Source {
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
    reconciled
}

pub(super) fn reconcile_requirements(
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
        let desired = desired_requirement(scope_id, owner, &address, &id, &declaration, source_ids);
        let (state, changes) = upsert_requirement(&mut records, desired);
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

pub(super) fn desired_requirement(
    scope_id: &ScopeId,
    owner: &str,
    address: &provenance_core::DeclarationAddress,
    id: &StableId,
    declaration: &TypedRequirementInput,
    source_ids: &BTreeMap<String, StableId>,
) -> Requirement {
    let source_refs = declaration
        .sources
        .iter()
        .map(|key| SourceReference {
            source_id: source_ids[key].clone(),
            clause: None,
        })
        .collect();
    Requirement {
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
        origin_thread: None,
        origin_message: None,
    }
}

fn upsert_requirement(
    records: &mut Vec<Requirement>,
    desired: Requirement,
) -> (ReconcileState, Vec<TypedFieldChange>) {
    let Some(existing) = records.iter_mut().find(|record| record.id == desired.id) else {
        records.push(desired);
        return (ReconcileState::Created, Vec::new());
    };
    let before = existing.clone();
    *existing = reconciled_requirement(&before, desired);
    let changes = requirement_changes(&before, existing);
    (
        state_after_change(before.declaration_address.as_ref(), existing, &before),
        changes,
    )
}

pub(super) fn reconciled_requirement(current: &Requirement, desired: Requirement) -> Requirement {
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
    reconciled
}

pub(super) fn reconcile_rules(
    mut records: Vec<Rule>,
    spec: &str,
    scope_id: &ScopeId,
    owner: &str,
    declarations: Vec<TypedRuleInput>,
    ids: &BTreeMap<provenance_core::DeclarationAddress, StableId>,
) -> anyhow::Result<(Vec<Rule>, Vec<ReconciledResource>)> {
    let mut resources = Vec::new();
    for declaration in declarations {
        let address = rule_address(spec, &declaration)?;
        let id = ids[&address].clone();
        let parent = local_parent(&address);
        let desired = desired_rule(scope_id, owner, &address, &id, &declaration);
        let (state, changes) = upsert_rule(&mut records, desired);
        resources.push(resource(
            TypedResourceKind::Rule,
            declaration.key,
            parent,
            address,
            id,
            state,
            changes,
        ));
    }
    retire_omitted_rules(&mut records, &mut resources, spec, owner, ids);
    records.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
    Ok((records, resources))
}

pub(super) fn desired_rule(
    scope_id: &ScopeId,
    owner: &str,
    address: &provenance_core::DeclarationAddress,
    id: &StableId,
    declaration: &TypedRuleInput,
) -> Rule {
    Rule {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope_id.clone(),
        id: id.clone(),
        declared_by: Some(owner.to_string()),
        declaration_address: Some(address.clone()),
        retired: false,
        name: declaration.name.clone(),
        description: declaration.description.clone(),
        statement: declaration.statement.clone(),
        status: RuleStatus::Active,
        severity: RuleSeverity::Medium,
        source_document: None,
        source_section: None,
        origin_thread: None,
        origin_message: None,
    }
}

fn upsert_rule(records: &mut Vec<Rule>, desired: Rule) -> (ReconcileState, Vec<TypedFieldChange>) {
    let Some(existing) = records.iter_mut().find(|record| record.id == desired.id) else {
        records.push(desired);
        return (ReconcileState::Created, Vec::new());
    };
    let before = existing.clone();
    *existing = reconciled_rule(&before, desired);
    let changes = rule_changes(&before, existing);
    (
        state_after_change(before.declaration_address.as_ref(), existing, &before),
        changes,
    )
}

pub(super) fn reconciled_rule(current: &Rule, desired: Rule) -> Rule {
    let mut reconciled = current.clone();
    reconciled.declared_by = desired.declared_by;
    reconciled.declaration_address = desired.declaration_address;
    reconciled.retired = false;
    reconciled.statement = desired.statement;
    if desired.name.is_some() {
        reconciled.name = desired.name;
    }
    if desired.description.is_some() {
        reconciled.description = desired.description;
    }
    reconciled
}

pub(super) fn source_changes(before: &Source, after: &Source) -> Vec<TypedFieldChange> {
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
    changes
}

pub(super) fn requirement_changes(
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
    changes
}

pub(super) fn rule_changes(before: &Rule, after: &Rule) -> Vec<TypedFieldChange> {
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
    changed(&mut changes, "name", &before.name, &after.name);
    changed(
        &mut changes,
        "description",
        &before.description,
        &after.description,
    );
    changes
}

fn changed<T: PartialEq + serde::Serialize>(
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

fn state_after_change<T: PartialEq + DeclarationRecord>(
    previous_address: Option<&provenance_core::DeclarationAddress>,
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

trait DeclarationRecord {
    fn declaration_address(&self) -> Option<&provenance_core::DeclarationAddress>;
}

impl DeclarationRecord for Source {
    fn declaration_address(&self) -> Option<&provenance_core::DeclarationAddress> {
        self.declaration_address.as_ref()
    }
}

impl DeclarationRecord for Requirement {
    fn declaration_address(&self) -> Option<&provenance_core::DeclarationAddress> {
        self.declaration_address.as_ref()
    }
}

impl DeclarationRecord for Rule {
    fn declaration_address(&self) -> Option<&provenance_core::DeclarationAddress> {
        self.declaration_address.as_ref()
    }
}

const fn resource(
    kind: TypedResourceKind,
    key: String,
    parent: Option<String>,
    address: provenance_core::DeclarationAddress,
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
