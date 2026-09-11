use super::{RequirementRelations, SaveRequirement};
use crate::{shards, state_store::StateStore};
use provenance_core::model::relations::RelationOwner;
use provenance_core::{Requirement, ScopeId, StableId};

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
    fn normalize(&mut self) {
        for ids in [&mut self.depends_on, &mut self.supersedes] {
            ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            ids.dedup();
        }
        self.source_refs.sort_by(|a, b| {
            a.source_id
                .as_str()
                .cmp(b.source_id.as_str())
                .then(a.clause.cmp(&b.clause))
        });
        self.source_refs.dedup();
    }
}

impl StateStore {
    pub(super) fn replace_review_relationships(
        &self,
        scope: &ScopeId,
        id: &StableId,
        relations: RequirementRelations,
    ) -> anyhow::Result<()> {
        let path = shards::requirements_path(&self.layout, scope);
        self.mutate_jsonl_records(&path, |records: &mut Vec<Requirement>| {
            let record = records.iter_mut().find(|r| r.id == *id).unwrap();
            record.refines = relations.refines;
            record.depends_on = relations.depends_on;
            record.supersedes = relations.supersedes;
            record.spawned_by = relations.spawned_by;
            record.source_refs = relations.source_refs;
            Ok(())
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
