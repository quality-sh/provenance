//! The named add, set, and clear methods, one per replaced reference.
//!
//! Each method is a one-line binding. The engine reads the record kind from
//! the store's own declaration, takes the record file from
//! `shards::path_for`, and reaches the field through the declaration-derived
//! `relation_slot_mut`, so a name here cannot drift away from the field it
//! names.
//!
//! Requirement references have no methods here: their one write path is the
//! guarded journal save, and the legacy relation commands are bridges over it
//! (`review::authoring`).

use super::StateStore;
use provenance_core::{Question, Resolution, Rule, ScopeId, Source, StableId};

impl StateStore {
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
