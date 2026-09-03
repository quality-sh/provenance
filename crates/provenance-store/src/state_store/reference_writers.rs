//! The add, set, and clear writers for reference fields.
//!
//! Every writer reaches the declaration through `RelationOwner`: the target
//! kind it checks, the requiredness a clear refuses against, and the cycle
//! guard on a requirement's own-kind fields all come from the table.

use super::StateStore;
use camino::Utf8Path;
use provenance_core::model::relations::{declaration_of, kind_word, RelationDecl, RelationOwner};
use provenance_core::{NodeType, ScopeId, StableId};
use serde::{de::DeserializeOwned, Serialize};

fn declared<T: RelationOwner>(name: &str) -> &'static RelationDecl {
    declaration_of(T::relations(), name).expect("every writer names a declared relation")
}

/// The flag a relation command names its owner with. Questions name their
/// owner `--id`; every other kind takes the kind word.
const fn owner_flag(kind: NodeType) -> &'static str {
    match kind {
        NodeType::Source => "--source-id",
        NodeType::Requirement => "--requirement-id",
        NodeType::Resolution => "--resolution-id",
        NodeType::Rule => "--rule-id",
        NodeType::Topic => "--topic-id",
        NodeType::Question => "--id",
        NodeType::Domain => "--domain-id",
        NodeType::Boundary => "--boundary-id",
    }
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
    /// Refuses an id no record of the kind holds. `named_by` is the
    /// user-facing slot the id came from: the flag on a command, the
    /// field on a declaration.
    pub(super) fn ensure_node_exists(
        &self,
        scope_id: &ScopeId,
        kind: NodeType,
        id: &StableId,
        named_by: &str,
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
        anyhow::ensure!(
            exists,
            "{} {} does not exist ({})",
            kind_word(kind),
            id.as_str(),
            named_by
        );
        Ok(())
    }

    /// Sets or clears a single reference field on one record.
    pub(super) fn write_single<T>(
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
                self.ensure_node_exists(scope_id, decl.target, target, "--target-id")?;
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
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "{} {} does not exist ({})",
                            kind_word(T::OWNER),
                            owner.as_str(),
                            owner_flag(T::OWNER)
                        )
                    })?;
                *field(record) = target;
                Ok(record.clone())
            })
        })
    }

    /// Adds one entry to a list field, sorted and without duplicates.
    pub(super) fn add_to_list<T>(
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
            self.ensure_node_exists(scope_id, decl.target, &target, "--target-id")?;
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
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "{} {} does not exist ({})",
                            kind_word(T::OWNER),
                            owner.as_str(),
                            owner_flag(T::OWNER)
                        )
                    })?;
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
    pub(super) fn clear_from_list<T>(
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
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "{} {} does not exist ({})",
                            kind_word(T::OWNER),
                            owner.as_str(),
                            owner_flag(T::OWNER)
                        )
                    })?;
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
