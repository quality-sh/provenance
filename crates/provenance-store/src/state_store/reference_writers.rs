//! The add, set, and clear writers for reference fields.
//!
//! Every writer reaches the declaration through `RelationOwner`: the target
//! kind it checks, the requiredness a clear refuses against, and the cycle
//! guard on a requirement's own-kind fields all come from the table. The
//! field it writes comes from the same table: `relation_slot_mut` lends the
//! slot the declaration names, so a writer cannot set one field under
//! another field's name.

use super::record_stamps::GraphRecord;
use super::StateStore;
use crate::shards;
use provenance_core::model::relations::{
    cycle_refusal, cycle_with_added_edges, declaration_of, kind_word, required_refusal,
    RelationDecl, RelationOwner, RelationSlot,
};
use provenance_core::{NodeType, ScopeId, StableId};
use provenance_macros::rule;
use std::collections::BTreeSet;

pub(super) fn declared<T: RelationOwner>(name: &str) -> &'static RelationDecl {
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

fn ensure_no_cycle<T: RelationOwner>(
    records: &[T],
    name: &str,
    owner: &StableId,
    targets: &[StableId],
) -> anyhow::Result<()> {
    if let Some(cycle) = cycle_with_added_edges(records, name, owner, targets) {
        return Err(crate::write_error::SourceFailure::wrap(
            crate::write_error::WriteFailure::InvalidUpdate,
            anyhow::anyhow!("{}", cycle_refusal(name, &cycle)),
        ));
    }
    Ok(())
}

impl StateStore {
    fn node_ids(&self, scope_id: &ScopeId, kind: NodeType) -> anyhow::Result<BTreeSet<String>> {
        macro_rules! ids {
            ($records:expr) => {
                $records?
                    .into_iter()
                    .map(|record| record.id.as_str().to_owned())
                    .collect()
            };
        }
        Ok(match kind {
            NodeType::Source => ids!(self.list_sources(scope_id)),
            NodeType::Requirement => ids!(self.list_requirements(scope_id)),
            NodeType::Resolution => ids!(self.list_resolutions(scope_id)),
            NodeType::Rule => ids!(self.list_rules(scope_id)),
            NodeType::Topic => ids!(self.list_topics(scope_id)),
            NodeType::Question => ids!(self.list_questions(scope_id)),
            NodeType::Domain => ids!(self.list_domains(scope_id)),
            NodeType::Boundary => ids!(self.list_boundaries(scope_id)),
        })
    }

    /// Refuses an id no record of the kind holds. `named_by` is the
    /// user-facing slot the id came from: the flag on a command, the
    /// field on a declaration.
    pub(crate) fn ensure_node_exists(
        &self,
        scope_id: &ScopeId,
        kind: NodeType,
        id: &StableId,
        named_by: &str,
    ) -> anyhow::Result<()> {
        let exists = self.node_ids(scope_id, kind)?.contains(id.as_str());
        crate::write_error::ensure!(
            MissingReference,
            exists,
            "{} {} does not exist ({})",
            kind_word(kind),
            id.as_str(),
            named_by
        );
        Ok(())
    }

    /// Checks each distinct named target before an edit can become a membership no-op.
    #[rule("rule_porcelain_relationship_noop_validates")]
    pub(crate) fn validate_relation_targets<T: RelationOwner>(
        &self,
        scope_id: &ScopeId,
        records: &[T],
        owner: &StableId,
        named_targets: &[(&str, &[StableId])],
    ) -> anyhow::Result<()> {
        let mut owner_ids = None;
        let mut target_indexes = Vec::new();
        for (name, targets) in named_targets {
            if targets.is_empty() {
                continue;
            }
            let declaration = declaration_of(T::relations(), name)
                .expect("every relationship edit names a declared relation");
            let mut seen = BTreeSet::new();
            let targets = targets
                .iter()
                .filter(|target| seen.insert(target.as_str().to_owned()))
                .cloned()
                .collect::<Vec<_>>();
            let kind = declaration.target;
            let ids = if kind == T::OWNER {
                owner_ids.get_or_insert_with(|| {
                    records
                        .iter()
                        .map(|record| record.id().as_str().to_owned())
                        .collect::<BTreeSet<_>>()
                })
            } else {
                let position = if let Some(position) = target_indexes
                    .iter()
                    .position(|(indexed_kind, _)| *indexed_kind == kind)
                {
                    position
                } else {
                    target_indexes.push((kind, self.node_ids(scope_id, kind)?));
                    target_indexes.len() - 1
                };
                &target_indexes[position].1
            };
            for target in &targets {
                crate::write_error::ensure!(
                    MissingReference,
                    ids.contains(target.as_str()),
                    "{} {} does not exist ({})",
                    kind_word(kind),
                    target.as_str(),
                    name
                );
            }
            if kind == T::OWNER {
                ensure_no_cycle(records, name, owner, &targets)?;
            }
        }
        Ok(())
    }

    /// Sets or clears a single reference field on one record.
    pub(super) fn write_single<T>(
        &self,
        scope_id: &ScopeId,
        name: &str,
        owner: &StableId,
        target: Option<StableId>,
    ) -> anyhow::Result<T>
    where
        T: GraphRecord,
    {
        let decl = declared::<T>(name);
        let path = shards::path_for(&self.layout, scope_id, T::OWNER);
        self.with_repository_publication(|| {
            if let Some(target) = &target {
                self.ensure_node_exists(scope_id, decl.target, target, "--target-id")?;
            }
            self.mutate_graph_record(&path, |records: &mut Vec<T>| {
                if let Some(target) = &target {
                    if decl.target == T::OWNER {
                        ensure_no_cycle(records, name, owner, std::slice::from_ref(target))?;
                    }
                }
                let record = records
                    .iter_mut()
                    .find(|record| record.id() == owner)
                    .ok_or_else(|| {
                        crate::write_error::SourceFailure::wrap(
                            crate::write_error::WriteFailure::MissingReference,
                            anyhow::anyhow!(
                                "{} {} does not exist ({})",
                                kind_word(T::OWNER),
                                owner.as_str(),
                                owner_flag(T::OWNER)
                            ),
                        )
                    })?;
                let Some(RelationSlot::Single(slot)) = record.relation_slot_mut(name) else {
                    panic!(
                        "relation `{name}` on {} is not a single reference",
                        kind_word(T::OWNER)
                    );
                };
                *slot = target;
                Ok(record.clone())
            })
        })
    }

    /// Adds one entry to a list field, sorted and without duplicates.
    pub(super) fn add_to_list<T>(
        &self,
        scope_id: &ScopeId,
        name: &str,
        owner: &StableId,
        target: StableId,
    ) -> anyhow::Result<T>
    where
        T: GraphRecord,
    {
        let decl = declared::<T>(name);
        let path = shards::path_for(&self.layout, scope_id, T::OWNER);
        self.with_repository_publication(|| {
            self.ensure_node_exists(scope_id, decl.target, &target, "--target-id")?;
            self.mutate_graph_record(&path, |records: &mut Vec<T>| {
                if decl.target == T::OWNER {
                    ensure_no_cycle(records, name, owner, std::slice::from_ref(&target))?;
                }
                let record = records
                    .iter_mut()
                    .find(|record| record.id() == owner)
                    .ok_or_else(|| {
                        crate::write_error::SourceFailure::wrap(
                            crate::write_error::WriteFailure::MissingReference,
                            anyhow::anyhow!(
                                "{} {} does not exist ({})",
                                kind_word(T::OWNER),
                                owner.as_str(),
                                owner_flag(T::OWNER)
                            ),
                        )
                    })?;
                let Some(RelationSlot::List(list)) = record.relation_slot_mut(name) else {
                    panic!(
                        "relation `{name}` on {} is not a reference list",
                        kind_word(T::OWNER)
                    );
                };
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
        scope_id: &ScopeId,
        name: &str,
        owner: &StableId,
        target: &StableId,
    ) -> anyhow::Result<T>
    where
        T: GraphRecord,
    {
        let decl = declared::<T>(name);
        let path = shards::path_for(&self.layout, scope_id, T::OWNER);
        self.with_repository_publication(|| {
            self.mutate_graph_record(&path, |records: &mut Vec<T>| {
                let position = records
                    .iter()
                    .position(|record| record.id() == owner)
                    .ok_or_else(|| {
                        crate::write_error::SourceFailure::wrap(
                            crate::write_error::WriteFailure::MissingReference,
                            anyhow::anyhow!(
                                "{} {} does not exist ({})",
                                kind_word(T::OWNER),
                                owner.as_str(),
                                owner_flag(T::OWNER)
                            ),
                        )
                    })?;
                self.validate_relation_targets(
                    scope_id,
                    records,
                    owner,
                    &[(name, std::slice::from_ref(target))],
                )?;
                let record = records
                    .get_mut(position)
                    .expect("the located relationship owner remains present");
                let Some(RelationSlot::List(list)) = record.relation_slot_mut(name) else {
                    panic!(
                        "relation `{name}` on {} is not a reference list",
                        kind_word(T::OWNER)
                    );
                };
                let Some(position) = list.iter().position(|entry| entry == target) else {
                    return Ok(record.clone());
                };
                crate::write_error::ensure!(
                    InvalidUpdate,
                    !(decl.required && list.len() == 1),
                    "{}",
                    required_refusal(decl)
                );
                list.remove(position);
                Ok(record.clone())
            })
        })
    }
}
