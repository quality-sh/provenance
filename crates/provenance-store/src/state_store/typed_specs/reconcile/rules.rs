use std::collections::BTreeMap;

use provenance_core::{
    DeclarationAddress, Rule, RuleSeverity, RuleStatus, ScopeId, StableId, SUPPORTED_SCHEMA_VERSION,
};

use super::super::super::{
    ReconcileState, ReconciledResource, TypedFieldChange, TypedResourceKind, TypedRuleInput,
};
use super::super::lifecycle::retire_omitted_rules;
use super::super::rule_addresses::{local_parent, rule_address};
use super::changes::{changed, resource, state_after_change};
use super::references;

pub(in crate::state_store::typed_specs) fn reconcile_rules(
    mut records: Vec<Rule>,
    spec: &str,
    scope_id: &ScopeId,
    owner: &str,
    declarations: Vec<TypedRuleInput>,
    ids: &BTreeMap<DeclarationAddress, StableId>,
    requirement_ids: &BTreeMap<String, StableId>,
) -> anyhow::Result<(Vec<Rule>, Vec<ReconciledResource>)> {
    let mut resources = Vec::new();
    for declaration in declarations {
        let address = rule_address(spec, &declaration)?;
        let id = ids[&address].clone();
        let parent = local_parent(&address);
        let desired = desired_rule(
            scope_id,
            owner,
            &address,
            &id,
            &declaration,
            requirement_ids,
        )?;
        let (state, changes) = upsert_rule(&mut records, desired, &declaration, requirement_ids)?;
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

pub(in crate::state_store::typed_specs) fn desired_rule(
    scope_id: &ScopeId,
    owner: &str,
    address: &DeclarationAddress,
    id: &StableId,
    declaration: &TypedRuleInput,
    requirement_ids: &BTreeMap<String, StableId>,
) -> anyhow::Result<Rule> {
    let mut rule = Rule {
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
        requirement_ids: Vec::new(),
        resolution_ids: Vec::new(),
        source_document: None,
        source_section: None,
        origin_thread: None,
        origin_message: None,
    };
    references::apply_rule(&mut rule, declaration, requirement_ids)?;
    Ok(rule)
}

fn upsert_rule(
    records: &mut Vec<Rule>,
    desired: Rule,
    declaration: &TypedRuleInput,
    requirement_ids: &BTreeMap<String, StableId>,
) -> anyhow::Result<(ReconcileState, Vec<TypedFieldChange>)> {
    let Some(existing) = records.iter_mut().find(|record| record.id == desired.id) else {
        records.push(desired);
        return Ok((ReconcileState::Created, Vec::new()));
    };
    let before = existing.clone();
    *existing = reconciled_rule(&before, desired, declaration, requirement_ids)?;
    let changes = rule_changes(&before, existing);
    Ok((
        state_after_change(before.declaration_address.as_ref(), existing, &before),
        changes,
    ))
}

/// The current record with the declaration's fields laid over it. The
/// requirement list is always the declaration's; `resolution_ids` stays as
/// it was when the declaration leaves it out.
pub(in crate::state_store::typed_specs) fn reconciled_rule(
    current: &Rule,
    desired: Rule,
    declaration: &TypedRuleInput,
    requirement_ids: &BTreeMap<String, StableId>,
) -> anyhow::Result<Rule> {
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
    references::apply_rule(&mut reconciled, declaration, requirement_ids)?;
    Ok(reconciled)
}

pub(in crate::state_store::typed_specs) fn rule_changes(
    before: &Rule,
    after: &Rule,
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
    changed(&mut changes, "name", &before.name, &after.name);
    changed(
        &mut changes,
        "description",
        &before.description,
        &after.description,
    );
    changed(
        &mut changes,
        "requirement_ids",
        &before.requirement_ids,
        &after.requirement_ids,
    );
    changed(
        &mut changes,
        "resolution_ids",
        &before.resolution_ids,
        &after.resolution_ids,
    );
    changes
}
