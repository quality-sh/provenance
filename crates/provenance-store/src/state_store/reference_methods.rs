//! The named add, set, and clear methods, one per replaced reference.
//!
//! Each names its field and hands the declaration's name to the generic
//! writers, which check the target kind, the requiredness, and the cycle.

use super::StateStore;
use crate::shards;
use provenance_core::{Question, Requirement, Resolution, Rule, ScopeId, Source, StableId};

impl StateStore {
    pub fn set_requirement_refines(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        target: StableId,
    ) -> anyhow::Result<Requirement> {
        let path = shards::requirements_path(&self.layout, scope_id);
        self.write_single(
            scope_id,
            &path,
            "refines",
            requirement,
            Some(target),
            |r: &mut Requirement| &mut r.refines,
        )
    }

    pub fn clear_requirement_refines(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
    ) -> anyhow::Result<Requirement> {
        let path = shards::requirements_path(&self.layout, scope_id);
        self.write_single(
            scope_id,
            &path,
            "refines",
            requirement,
            None,
            |r: &mut Requirement| &mut r.refines,
        )
    }

    pub fn add_requirement_depends_on(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        target: StableId,
    ) -> anyhow::Result<Requirement> {
        let path = shards::requirements_path(&self.layout, scope_id);
        self.add_to_list(
            scope_id,
            &path,
            "depends_on",
            requirement,
            target,
            |r: &mut Requirement| &mut r.depends_on,
        )
    }

    pub fn clear_requirement_depends_on(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        target: &StableId,
    ) -> anyhow::Result<Requirement> {
        let path = shards::requirements_path(&self.layout, scope_id);
        self.clear_from_list(
            &path,
            "depends_on",
            requirement,
            target,
            |r: &mut Requirement| &mut r.depends_on,
        )
    }

    pub fn add_requirement_supersedes(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        target: StableId,
    ) -> anyhow::Result<Requirement> {
        let path = shards::requirements_path(&self.layout, scope_id);
        self.add_to_list(
            scope_id,
            &path,
            "supersedes",
            requirement,
            target,
            |r: &mut Requirement| &mut r.supersedes,
        )
    }

    pub fn clear_requirement_supersedes(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        target: &StableId,
    ) -> anyhow::Result<Requirement> {
        let path = shards::requirements_path(&self.layout, scope_id);
        self.clear_from_list(
            &path,
            "supersedes",
            requirement,
            target,
            |r: &mut Requirement| &mut r.supersedes,
        )
    }

    pub fn set_requirement_spawned_by(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        target: StableId,
    ) -> anyhow::Result<Requirement> {
        let path = shards::requirements_path(&self.layout, scope_id);
        self.write_single(
            scope_id,
            &path,
            "spawned_by",
            requirement,
            Some(target),
            |r: &mut Requirement| &mut r.spawned_by,
        )
    }

    pub fn clear_requirement_spawned_by(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
    ) -> anyhow::Result<Requirement> {
        let path = shards::requirements_path(&self.layout, scope_id);
        self.write_single(
            scope_id,
            &path,
            "spawned_by",
            requirement,
            None,
            |r: &mut Requirement| &mut r.spawned_by,
        )
    }

    /// Removes every citation of one source from a requirement.
    pub fn clear_source_reference(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        source: &StableId,
    ) -> anyhow::Result<Requirement> {
        let path = shards::requirements_path(&self.layout, scope_id);
        self.with_repository_publication(|| {
            self.mutate_jsonl_records(&path, |records: &mut Vec<Requirement>| {
                let record = records
                    .iter_mut()
                    .find(|record| &record.id == requirement)
                    .ok_or_else(|| anyhow::anyhow!("requirement does not exist"))?;
                anyhow::ensure!(
                    record
                        .source_refs
                        .iter()
                        .any(|entry| &entry.source_id == source),
                    "requirement {} does not cite source {}",
                    requirement.as_str(),
                    source.as_str()
                );
                record
                    .source_refs
                    .retain(|entry| &entry.source_id != source);
                Ok(record.clone())
            })
        })
    }

    pub fn add_rule_requirement(
        &self,
        scope_id: &ScopeId,
        rule: &StableId,
        target: StableId,
    ) -> anyhow::Result<Rule> {
        let path = shards::rules_path(&self.layout, scope_id);
        self.add_to_list(
            scope_id,
            &path,
            "requirement_ids",
            rule,
            target,
            |r: &mut Rule| &mut r.requirement_ids,
        )
    }

    pub fn clear_rule_requirement(
        &self,
        scope_id: &ScopeId,
        rule: &StableId,
        target: &StableId,
    ) -> anyhow::Result<Rule> {
        let path = shards::rules_path(&self.layout, scope_id);
        self.clear_from_list(&path, "requirement_ids", rule, target, |r: &mut Rule| {
            &mut r.requirement_ids
        })
    }

    pub fn add_rule_resolution(
        &self,
        scope_id: &ScopeId,
        rule: &StableId,
        target: StableId,
    ) -> anyhow::Result<Rule> {
        let path = shards::rules_path(&self.layout, scope_id);
        self.add_to_list(
            scope_id,
            &path,
            "resolution_ids",
            rule,
            target,
            |r: &mut Rule| &mut r.resolution_ids,
        )
    }

    pub fn clear_rule_resolution(
        &self,
        scope_id: &ScopeId,
        rule: &StableId,
        target: &StableId,
    ) -> anyhow::Result<Rule> {
        let path = shards::rules_path(&self.layout, scope_id);
        self.clear_from_list(&path, "resolution_ids", rule, target, |r: &mut Rule| {
            &mut r.resolution_ids
        })
    }

    pub fn add_resolution_requirement(
        &self,
        scope_id: &ScopeId,
        resolution: &StableId,
        target: StableId,
    ) -> anyhow::Result<Resolution> {
        let path = shards::resolutions_path(&self.layout, scope_id);
        self.add_to_list(
            scope_id,
            &path,
            "requirement_ids",
            resolution,
            target,
            |r: &mut Resolution| &mut r.requirement_ids,
        )
    }

    pub fn clear_resolution_requirement(
        &self,
        scope_id: &ScopeId,
        resolution: &StableId,
        target: &StableId,
    ) -> anyhow::Result<Resolution> {
        let path = shards::resolutions_path(&self.layout, scope_id);
        self.clear_from_list(
            &path,
            "requirement_ids",
            resolution,
            target,
            |r: &mut Resolution| &mut r.requirement_ids,
        )
    }

    pub fn add_resolution_supersedes(
        &self,
        scope_id: &ScopeId,
        resolution: &StableId,
        target: StableId,
    ) -> anyhow::Result<Resolution> {
        let path = shards::resolutions_path(&self.layout, scope_id);
        self.add_to_list(
            scope_id,
            &path,
            "supersedes",
            resolution,
            target,
            |r: &mut Resolution| &mut r.supersedes,
        )
    }

    pub fn clear_resolution_supersedes(
        &self,
        scope_id: &ScopeId,
        resolution: &StableId,
        target: &StableId,
    ) -> anyhow::Result<Resolution> {
        let path = shards::resolutions_path(&self.layout, scope_id);
        self.clear_from_list(
            &path,
            "supersedes",
            resolution,
            target,
            |r: &mut Resolution| &mut r.supersedes,
        )
    }

    pub fn add_source_supersedes(
        &self,
        scope_id: &ScopeId,
        source: &StableId,
        target: StableId,
    ) -> anyhow::Result<Source> {
        let path = shards::sources_path(&self.layout, scope_id);
        self.add_to_list(
            scope_id,
            &path,
            "supersedes",
            source,
            target,
            |r: &mut Source| &mut r.supersedes,
        )
    }

    pub fn clear_source_supersedes(
        &self,
        scope_id: &ScopeId,
        source: &StableId,
        target: &StableId,
    ) -> anyhow::Result<Source> {
        let path = shards::sources_path(&self.layout, scope_id);
        self.clear_from_list(&path, "supersedes", source, target, |r: &mut Source| {
            &mut r.supersedes
        })
    }

    pub fn set_question_contradicts(
        &self,
        scope_id: &ScopeId,
        question: &StableId,
        target: StableId,
    ) -> anyhow::Result<Question> {
        let path = shards::questions_path(&self.layout, scope_id);
        self.write_single(
            scope_id,
            &path,
            "contradicts",
            question,
            Some(target),
            |r: &mut Question| &mut r.contradicts,
        )
    }

    pub fn clear_question_contradicts(
        &self,
        scope_id: &ScopeId,
        question: &StableId,
    ) -> anyhow::Result<Question> {
        let path = shards::questions_path(&self.layout, scope_id);
        self.write_single(
            scope_id,
            &path,
            "contradicts",
            question,
            None,
            |r: &mut Question| &mut r.contradicts,
        )
    }
}
