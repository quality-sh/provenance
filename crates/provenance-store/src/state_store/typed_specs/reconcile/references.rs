//! The reference fields a declaration may set.
//!
//! A field the declaration names is authoritative; a field it leaves out is
//! untouched, so a CLI-set `refines` survives a spec that does not name one.
//! `source_refs` keeps its append. Canonical ids a declaration names
//! (`spawned_by`, `resolution_ids`) must exist; `refines`, `depends_on`, and
//! `supersedes` name keys of the same document.

use std::collections::BTreeMap;

use provenance_core::model::relations::RelationOwner;
use provenance_core::{NodeType, Requirement, Rule, ScopeId, Source, StableId};

use super::super::super::{StateStore, TypedRequirementInput, TypedRuleInput, TypedSourceInput};

fn ids_of(keys: &[String], ids: &BTreeMap<String, StableId>) -> Vec<StableId> {
    let mut resolved: Vec<StableId> = keys.iter().map(|key| ids[key].clone()).collect();
    resolved.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    resolved.dedup();
    resolved
}

fn canonical_ids(ids: &[String]) -> anyhow::Result<Vec<StableId>> {
    let mut resolved = ids
        .iter()
        .map(|id| StableId::new(id.clone()))
        .collect::<anyhow::Result<Vec<_>>>()?;
    resolved.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    resolved.dedup();
    Ok(resolved)
}

pub(super) fn apply_source(
    record: &mut Source,
    declaration: &TypedSourceInput,
    source_ids: &BTreeMap<String, StableId>,
) {
    if let Some(keys) = &declaration.supersedes {
        record.supersedes = ids_of(keys, source_ids);
    }
}

pub(super) fn apply_requirement(
    record: &mut Requirement,
    declaration: &TypedRequirementInput,
    requirement_ids: &BTreeMap<String, StableId>,
) -> anyhow::Result<()> {
    if let Some(key) = &declaration.refines {
        record.refines = Some(requirement_ids[key].clone());
    }
    if let Some(keys) = &declaration.depends_on {
        record.depends_on = ids_of(keys, requirement_ids);
    }
    if let Some(keys) = &declaration.supersedes {
        record.supersedes = ids_of(keys, requirement_ids);
    }
    if let Some(resolution) = &declaration.spawned_by {
        record.spawned_by = Some(StableId::new(resolution.clone())?);
    }
    Ok(())
}

pub(super) fn apply_rule(
    record: &mut Rule,
    declaration: &TypedRuleInput,
    requirement_ids: &BTreeMap<String, StableId>,
) -> anyhow::Result<()> {
    record.requirement_ids = ids_of(&declaration.requirements, requirement_ids);
    if let Some(resolutions) = &declaration.resolution_ids {
        record.resolution_ids = canonical_ids(resolutions)?;
    }
    Ok(())
}

/// The canonical resolutions a document names must exist in the scope.
pub(in crate::state_store::typed_specs) fn ensure_resolutions_exist(
    store: &StateStore,
    scope_id: &ScopeId,
    requirements: &[Requirement],
    rules: &[Rule],
) -> anyhow::Result<()> {
    for resolution in requirements
        .iter()
        .filter_map(|requirement| requirement.spawned_by.as_ref())
    {
        store.ensure_node_exists(scope_id, NodeType::Resolution, resolution, "spawned_by")?;
    }
    for resolution in rules.iter().flat_map(|rule| &rule.resolution_ids) {
        store.ensure_node_exists(scope_id, NodeType::Resolution, resolution, "resolution_ids")?;
    }
    Ok(())
}

/// Refuses a `refines`, `depends_on`, or `supersedes` chain that returns to
/// its start.
pub(in crate::state_store::typed_specs) fn ensure_acyclic(
    requirements: &[Requirement],
) -> anyhow::Result<()> {
    for name in ["refines", "depends_on", "supersedes"] {
        for start in requirements {
            let mut stack: Vec<&StableId> = targets(start, name);
            let mut seen = Vec::new();
            while let Some(current) = stack.pop() {
                anyhow::ensure!(
                    current != start.id(),
                    "{name} from {} returns to itself",
                    start.id().as_str()
                );
                if seen.contains(&current) {
                    continue;
                }
                seen.push(current);
                if let Some(next) = requirements.iter().find(|record| record.id() == current) {
                    stack.extend(targets(next, name));
                }
            }
        }
    }
    Ok(())
}

fn targets<'a>(record: &'a Requirement, name: &str) -> Vec<&'a StableId> {
    record
        .references()
        .into_iter()
        .filter(|(relation, _)| *relation == name)
        .map(|(_, id)| id)
        .collect()
}
