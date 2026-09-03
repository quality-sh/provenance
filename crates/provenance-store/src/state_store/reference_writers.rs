//! The add, set, and clear writers for reference fields.
//!
//! Every writer reaches the declaration through `RelationOwner`: the target
//! kind it checks, the requiredness a clear refuses against, and the cycle
//! guard on a requirement's own-kind fields all come from the table.

use super::StateStore;
use crate::shards;
use camino::Utf8Path;
use provenance_core::model::relations::{declaration_of, RelationDecl, RelationOwner};
use provenance_core::{
    NodeType, Question, Requirement, Resolution, Rule, ScopeId, Source, StableId,
};
use serde::{de::DeserializeOwned, Serialize};

/// The word a refusal uses for a record kind.
pub(super) const fn kind_word(kind: NodeType) -> &'static str {
    match kind {
        NodeType::Source => "source",
        NodeType::Requirement => "requirement",
        NodeType::Resolution => "resolution",
        NodeType::Rule => "rule",
        NodeType::Topic => "topic",
        NodeType::Question => "question",
        NodeType::Domain => "domain",
        NodeType::Boundary => "boundary",
    }
}

fn declared<T: RelationOwner>(name: &str) -> &'static RelationDecl {
    declaration_of(T::relations(), name).expect("every writer names a declared relation")
}

/// A `refines`, `depends_on`, or `supersedes` chain that leads back to the owner.
fn forms_cycle<T: RelationOwner>(
    records: &[T],
    name: &str,
    owner: &StableId,
    target: &StableId,
) -> bool {
    let mut stack = vec![target.clone()];
    let mut seen = Vec::new();
    while let Some(current) = stack.pop() {
        if current == *owner {
            return true;
        }
        if seen.contains(&current) {
            continue;
        }
        seen.push(current.clone());
        if let Some(record) = records.iter().find(|record| *record.id() == current) {
            stack.extend(
                record
                    .references()
                    .into_iter()
                    .filter(|(relation, _)| *relation == name)
                    .map(|(_, id)| id.clone()),
            );
        }
    }
    false
}

impl StateStore {
    pub(super) fn ensure_node_exists(
        &self,
        scope_id: &ScopeId,
        kind: NodeType,
        id: &StableId,
    ) -> anyhow::Result<()> {
        let exists = match kind {
            NodeType::Source => self.list_sources(scope_id)?.iter().any(|r| &r.id == id),
            NodeType::Requirement => self
                .list_requirements(scope_id)?
                .iter()
                .any(|r| &r.id == id),
            NodeType::Resolution => self.list_resolutions(scope_id)?.iter().any(|r| &r.id == id),
            NodeType::Rule => self.list_rules(scope_id)?.iter().any(|r| &r.id == id),
            NodeType::Topic => self.list_topics(scope_id)?.iter().any(|r| &r.id == id),
            NodeType::Question => self.list_questions(scope_id)?.iter().any(|r| &r.id == id),
            NodeType::Domain => self.list_domains(scope_id)?.iter().any(|r| &r.id == id),
            NodeType::Boundary => self.list_boundaries(scope_id)?.iter().any(|r| &r.id == id),
        };
        anyhow::ensure!(exists, "{} does not exist", kind_word(kind));
        Ok(())
    }

    /// Sets or clears a single reference field on one record.
    fn write_single<T>(
        &self,
        scope_id: &ScopeId,
        path: &Utf8Path,
        name: &str,
        owner: &StableId,
        target: Option<StableId>,
        field: impl FnOnce(&mut T) -> &mut Option<StableId>,
    ) -> anyhow::Result<T>
    where
        T: RelationOwner + DeserializeOwned + Serialize + Clone,
    {
        let decl = declared::<T>(name);
        self.with_repository_publication(|| {
            if let Some(target) = &target {
                self.ensure_node_exists(scope_id, decl.target, target)?;
            }
            self.mutate_jsonl_records(path, |records: &mut Vec<T>| {
                if let Some(target) = &target {
                    if decl.target == T::OWNER {
                        anyhow::ensure!(
                            !forms_cycle(records, name, owner, target),
                            "{name} from {} to {} would form a cycle",
                            owner.as_str(),
                            target.as_str()
                        );
                    }
                }
                let record = records
                    .iter_mut()
                    .find(|record| record.id() == owner)
                    .ok_or_else(|| anyhow::anyhow!("{} does not exist", kind_word(T::OWNER)))?;
                *field(record) = target;
                Ok(record.clone())
            })
        })
    }

    /// Adds one entry to a list field, sorted and without duplicates.
    fn add_to_list<T>(
        &self,
        scope_id: &ScopeId,
        path: &Utf8Path,
        name: &str,
        owner: &StableId,
        target: StableId,
        field: impl FnOnce(&mut T) -> &mut Vec<StableId>,
    ) -> anyhow::Result<T>
    where
        T: RelationOwner + DeserializeOwned + Serialize + Clone,
    {
        let decl = declared::<T>(name);
        self.with_repository_publication(|| {
            self.ensure_node_exists(scope_id, decl.target, &target)?;
            self.mutate_jsonl_records(path, |records: &mut Vec<T>| {
                if decl.target == T::OWNER {
                    anyhow::ensure!(
                        !forms_cycle(records, name, owner, &target),
                        "{name} from {} to {} would form a cycle",
                        owner.as_str(),
                        target.as_str()
                    );
                }
                let record = records
                    .iter_mut()
                    .find(|record| record.id() == owner)
                    .ok_or_else(|| anyhow::anyhow!("{} does not exist", kind_word(T::OWNER)))?;
                let list = field(record);
                if !list.contains(&target) {
                    list.push(target);
                    list.sort_by(|a, b| a.as_str().cmp(b.as_str()));
                }
                Ok(record.clone())
            })
        })
    }

    /// Removes one entry from a list field; a required list keeps its last.
    fn clear_from_list<T>(
        &self,
        path: &Utf8Path,
        name: &str,
        owner: &StableId,
        target: &StableId,
        field: impl FnOnce(&mut T) -> &mut Vec<StableId>,
    ) -> anyhow::Result<T>
    where
        T: RelationOwner + DeserializeOwned + Serialize + Clone,
    {
        let decl = declared::<T>(name);
        self.with_repository_publication(|| {
            self.mutate_jsonl_records(path, |records: &mut Vec<T>| {
                let record = records
                    .iter_mut()
                    .find(|record| record.id() == owner)
                    .ok_or_else(|| anyhow::anyhow!("{} does not exist", kind_word(T::OWNER)))?;
                let list = field(record);
                let position = list
                    .iter()
                    .position(|entry| entry == target)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "{} {} does not name {} {}",
                            kind_word(T::OWNER),
                            owner.as_str(),
                            kind_word(decl.target),
                            target.as_str()
                        )
                    })?;
                anyhow::ensure!(
                    !(decl.required && list.len() == 1),
                    "a {} needs one {}",
                    kind_word(T::OWNER),
                    kind_word(decl.target)
                );
                list.remove(position);
                Ok(record.clone())
            })
        })
    }
}

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
