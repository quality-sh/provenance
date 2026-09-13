use super::input::{CitesEdit, ListEdit, RequirementRelations, SaveRequirement, SingleEdit};
use crate::{shards, state_store::StateStore};
use provenance_core::model::relations::RelationOwner;
use provenance_core::{Requirement, ScopeId, SourceReference, StableId};

/// The complete final relationship sets one save publishes.
#[derive(Debug, Clone, Default)]
pub(super) struct FinalRelations {
    pub refines: Option<StableId>,
    pub depends_on: Vec<StableId>,
    pub supersedes: Vec<StableId>,
    pub spawned_by: Option<StableId>,
    pub cites: Vec<SourceReference>,
}

impl SaveRequirement {
    pub(super) fn normalize(&mut self) {
        self.update
            .clear_fields
            .sort_by_key(|field| format!("{field:?}"));
        self.update.clear_fields.dedup();
        if let Some(relations) = &mut self.relationships {
            relations.normalize();
        }
    }
}

impl RequirementRelations {
    /// Sorts and dedups every set, so the same edit always carries the same
    /// canonical intent and request identity stays deterministic.
    fn normalize(&mut self) {
        sort_list(self.depends_on.as_mut());
        sort_list(self.supersedes.as_mut());
        sort_cites(self.cites.as_mut());
    }

    /// Expands the edit against the record's current state into complete
    /// final sets. Expansion is deterministic: the same edit on the same
    /// record always yields the same final state, so request identity and
    /// idempotency resolution stay correct.
    pub(super) fn expand(&self, before: &Requirement) -> anyhow::Result<FinalRelations> {
        let mut final_sets = FinalRelations {
            refines: before.refines.clone(),
            depends_on: before.depends_on.clone(),
            supersedes: before.supersedes.clone(),
            spawned_by: before.spawned_by.clone(),
            cites: before.source_refs.clone(),
        };
        if let Some(edit) = &self.refines {
            final_sets.refines = match edit {
                SingleEdit::Set(id) => Some(id.clone()),
                SingleEdit::Clear => None,
            };
        }
        expand_list(
            &mut final_sets.depends_on,
            "depends_on",
            before,
            self.depends_on.as_ref(),
        )?;
        expand_list(
            &mut final_sets.supersedes,
            "supersedes",
            before,
            self.supersedes.as_ref(),
        )?;
        if let Some(edit) = &self.spawned_by {
            final_sets.spawned_by = match edit {
                SingleEdit::Set(id) => Some(id.clone()),
                SingleEdit::Clear => None,
            };
        }
        expand_cites(&mut final_sets.cites, before, self.cites.as_ref())?;
        Ok(final_sets)
    }
}

fn sort_list(edit: Option<&mut ListEdit>) {
    let Some(edit) = edit else { return };
    match edit {
        ListEdit::Set(entries) => sort_ids(entries),
        ListEdit::Delta { add, remove } => {
            sort_ids(add);
            sort_ids(remove);
        }
    }
}

fn sort_cites(edit: Option<&mut CitesEdit>) {
    let Some(edit) = edit else { return };
    match edit {
        CitesEdit::Set(entries) => sort_citations(entries),
        CitesEdit::Delta { add, remove } => {
            sort_citations(add);
            sort_ids(remove);
        }
    }
}

fn sort_ids(entries: &mut [StableId]) {
    entries.sort_by(|a, b| a.as_str().cmp(b.as_str()));
}

pub(super) fn sorted_ids(mut ids: Vec<StableId>) -> Vec<StableId> {
    sort_ids(&mut ids);
    ids.dedup();
    ids
}

fn sorted_citations(mut refs: Vec<SourceReference>) -> Vec<SourceReference> {
    refs.sort_by(|a, b| {
        a.source_id
            .as_str()
            .cmp(b.source_id.as_str())
            .then(a.clause.cmp(&b.clause))
    });
    refs.dedup();
    refs
}

fn sort_citations(entries: &mut [SourceReference]) {
    entries.sort_by(|a, b| {
        a.source_id
            .as_str()
            .cmp(b.source_id.as_str())
            .then(a.clause.cmp(&b.clause))
    });
}

fn expand_list(
    target: &mut Vec<StableId>,
    name: &str,
    before: &Requirement,
    edit: Option<&ListEdit>,
) -> anyhow::Result<()> {
    let Some(edit) = edit else { return Ok(()) };
    match edit {
        ListEdit::Set(entries) => *target = sorted_ids(entries.clone()),
        ListEdit::Delta { add, remove } => {
            for entry in add.clone() {
                if !target.contains(&entry) {
                    target.push(entry);
                }
            }
            for entry in remove {
                anyhow::ensure!(
                    target.contains(entry),
                    "requirement {} does not name a record under {name}: {}",
                    before.id.as_str(),
                    entry.as_str()
                );
            }
            target.retain(|entry| !remove.contains(entry));
            *target = sorted_ids(std::mem::take(target));
        }
    }
    Ok(())
}

fn expand_cites(
    target: &mut Vec<SourceReference>,
    before: &Requirement,
    edit: Option<&CitesEdit>,
) -> anyhow::Result<()> {
    let Some(edit) = edit else { return Ok(()) };
    match edit {
        CitesEdit::Set(entries) => *target = sorted_citations(entries.clone()),
        CitesEdit::Delta { add, remove } => {
            for citation in add.clone() {
                if !target.contains(&citation) {
                    target.push(citation);
                }
            }
            for source in remove {
                anyhow::ensure!(
                    target.iter().any(|entry| &entry.source_id == source),
                    "requirement {} does not name source {} under cites",
                    before.id.as_str(),
                    source.as_str()
                );
            }
            target.retain(|entry| !remove.contains(&entry.source_id));
            *target = sorted_citations(std::mem::take(target));
        }
    }
    Ok(())
}

impl StateStore {
    pub(super) fn replace_review_relationships(
        &self,
        scope: &ScopeId,
        id: &StableId,
        final_sets: FinalRelations,
    ) -> anyhow::Result<()> {
        let path = shards::requirements_path(&self.layout, scope);
        self.mutate_graph_record(&path, |records: &mut Vec<Requirement>| {
            let record = records.iter_mut().find(|r| r.id == *id).unwrap();
            record.refines = final_sets.refines;
            record.depends_on = final_sets.depends_on;
            record.supersedes = final_sets.supersedes;
            record.spawned_by = final_sets.spawned_by;
            record.source_refs = final_sets.cites;
            Ok(record.clone())
        })?;
        let record = self.requirement(scope, id)?;
        for (name, target) in record.references() {
            let decl = Requirement::relations()
                .iter()
                .find(|d| d.name == name)
                .unwrap();
            self.ensure_node_exists(scope, decl.target, target, name)?;
        }
        self.validate_graph_scope(scope)
    }
}
