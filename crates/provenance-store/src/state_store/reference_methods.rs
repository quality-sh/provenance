//! The named add, set, and clear methods, one per replaced reference.
//!
//! Each method is a one-line binding. The engine reads the record kind from
//! the store's own declaration, takes the record file from
//! `shards::path_for`, and reaches the field through the declaration-derived
//! `relation_slot_mut`, so a name here cannot drift away from the field it
//! names.

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
        self.write_single(scope_id, "refines", requirement, Some(target))
    }

    pub fn clear_requirement_refines(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
    ) -> anyhow::Result<Requirement> {
        self.write_single(scope_id, "refines", requirement, None)
    }

    pub fn add_requirement_depends_on(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        target: StableId,
    ) -> anyhow::Result<Requirement> {
        self.add_to_list(scope_id, "depends_on", requirement, target)
    }

    pub fn clear_requirement_depends_on(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        target: &StableId,
    ) -> anyhow::Result<Requirement> {
        self.clear_from_list(scope_id, "depends_on", requirement, target)
    }

    pub fn add_requirement_supersedes(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        target: StableId,
    ) -> anyhow::Result<Requirement> {
        self.add_to_list(scope_id, "supersedes", requirement, target)
    }

    pub fn clear_requirement_supersedes(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        target: &StableId,
    ) -> anyhow::Result<Requirement> {
        self.clear_from_list(scope_id, "supersedes", requirement, target)
    }

    pub fn set_requirement_spawned_by(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        target: StableId,
    ) -> anyhow::Result<Requirement> {
        self.write_single(scope_id, "spawned_by", requirement, Some(target))
    }

    pub fn clear_requirement_spawned_by(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
    ) -> anyhow::Result<Requirement> {
        self.write_single(scope_id, "spawned_by", requirement, None)
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
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "requirement {} does not exist (--requirement-id)",
                            requirement.as_str()
                        )
                    })?;
                anyhow::ensure!(
                    record
                        .source_refs
                        .iter()
                        .any(|entry| &entry.source_id == source),
                    "requirement {} does not name source {} under cites",
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
        self.add_to_list(scope_id, "requirement_ids", rule, target)
    }

    pub fn clear_rule_requirement(
        &self,
        scope_id: &ScopeId,
        rule: &StableId,
        target: &StableId,
    ) -> anyhow::Result<Rule> {
        self.clear_from_list(scope_id, "requirement_ids", rule, target)
    }

    pub fn add_rule_resolution(
        &self,
        scope_id: &ScopeId,
        rule: &StableId,
        target: StableId,
    ) -> anyhow::Result<Rule> {
        self.add_to_list(scope_id, "resolution_ids", rule, target)
    }

    pub fn clear_rule_resolution(
        &self,
        scope_id: &ScopeId,
        rule: &StableId,
        target: &StableId,
    ) -> anyhow::Result<Rule> {
        self.clear_from_list(scope_id, "resolution_ids", rule, target)
    }

    pub fn add_resolution_requirement(
        &self,
        scope_id: &ScopeId,
        resolution: &StableId,
        target: StableId,
    ) -> anyhow::Result<Resolution> {
        self.add_to_list(scope_id, "requirement_ids", resolution, target)
    }

    pub fn clear_resolution_requirement(
        &self,
        scope_id: &ScopeId,
        resolution: &StableId,
        target: &StableId,
    ) -> anyhow::Result<Resolution> {
        self.clear_from_list(scope_id, "requirement_ids", resolution, target)
    }

    pub fn add_resolution_supersedes(
        &self,
        scope_id: &ScopeId,
        resolution: &StableId,
        target: StableId,
    ) -> anyhow::Result<Resolution> {
        self.add_to_list(scope_id, "supersedes", resolution, target)
    }

    pub fn clear_resolution_supersedes(
        &self,
        scope_id: &ScopeId,
        resolution: &StableId,
        target: &StableId,
    ) -> anyhow::Result<Resolution> {
        self.clear_from_list(scope_id, "supersedes", resolution, target)
    }

    pub fn add_source_supersedes(
        &self,
        scope_id: &ScopeId,
        source: &StableId,
        target: StableId,
    ) -> anyhow::Result<Source> {
        self.add_to_list(scope_id, "supersedes", source, target)
    }

    pub fn clear_source_supersedes(
        &self,
        scope_id: &ScopeId,
        source: &StableId,
        target: &StableId,
    ) -> anyhow::Result<Source> {
        self.clear_from_list(scope_id, "supersedes", source, target)
    }

    pub fn set_question_contradicts(
        &self,
        scope_id: &ScopeId,
        question: &StableId,
        target: StableId,
    ) -> anyhow::Result<Question> {
        self.write_single(scope_id, "contradicts", question, Some(target))
    }

    pub fn clear_question_contradicts(
        &self,
        scope_id: &ScopeId,
        question: &StableId,
    ) -> anyhow::Result<Question> {
        self.write_single(scope_id, "contradicts", question, None)
    }
}
