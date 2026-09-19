//! Bucket tables over one loaded graph.
//!
//! The index is an implementation detail of [`super::graph_query`]: it is
//! built once when a `GraphQuery` is constructed and never leaves the
//! gaps module. Every table maps a stable id to the record positions
//! that carry it, so a lookup costs one hash probe plus the records it
//! answers with, never a scan of the record vectors.
//!
//! Record order is preserved everywhere: owner positions enter each
//! table in record order, so an inverse join reads back in owner record
//! order, and a forward join collects candidate positions and sorts
//! them, so it reads back in target record order.

use super::graph_query::GapGraph;
use provenance_core::model::relations::RelationOwner;
use provenance_core::{NodeType, Requirement, Resolution, Rule, StableId};
use std::collections::{BTreeSet, HashMap};

pub(super) struct GraphIndex<'graph> {
    /// Every record position per node kind, keyed by stable id. A bucket
    /// holds one position per record, in record order, so two records
    /// sharing an id answer a join as two rows.
    ids: IdTables<'graph>,
    requirements: &'graph [Requirement],
    resolutions: &'graph [Resolution],
    rules: &'graph [Rule],
    /// Resolutions whose `requirement_ids` name the requirement.
    resolutions_by_requirement: HashMap<&'graph str, Vec<usize>>,
    /// Rules naming the requirement directly.
    rules_by_requirement: HashMap<&'graph str, Vec<usize>>,
    /// Rules naming the resolution.
    rules_by_resolution: HashMap<&'graph str, Vec<usize>>,
    /// Requirements whose `refines` names the parent.
    children_by_parent: HashMap<&'graph str, Vec<usize>>,
    /// Requirements whose `supersedes` names the target.
    requirements_by_superseded: HashMap<&'graph str, Vec<usize>>,
    /// Requirements whose `spawned_by` names the resolution.
    requirements_by_spawn: HashMap<&'graph str, Vec<usize>>,
    /// Requirements citing the source under `source_refs`.
    requirements_by_source: HashMap<&'graph str, Vec<usize>>,
    /// Resolutions whose `supersedes` names the target.
    resolutions_by_superseded: HashMap<&'graph str, Vec<usize>>,
    /// Sources whose `supersedes` names the target.
    sources_by_superseded: HashMap<&'graph str, Vec<usize>>,
    /// Rule id to the requirements that rule answers to, inverted from
    /// the forward traversal: one walk over the requirements, read in
    /// both directions.
    attribution: HashMap<&'graph str, Vec<&'graph Requirement>>,
}

impl<'graph> GraphIndex<'graph> {
    pub(super) fn new(graph: &GapGraph<'graph>) -> Self {
        let mut index = Self {
            ids: IdTables::new(graph),
            requirements: graph.requirements,
            resolutions: graph.resolutions,
            rules: graph.rules,
            resolutions_by_requirement: inverse_table(graph.resolutions, "requirement_ids"),
            rules_by_requirement: inverse_table(graph.rules, "requirement_ids"),
            rules_by_resolution: inverse_table(graph.rules, "resolution_ids"),
            children_by_parent: inverse_table(graph.requirements, "refines"),
            requirements_by_superseded: inverse_table(graph.requirements, "supersedes"),
            requirements_by_spawn: inverse_table(graph.requirements, "spawned_by"),
            requirements_by_source: inverse_table(graph.requirements, "cites"),
            resolutions_by_superseded: inverse_table(graph.resolutions, "supersedes"),
            sources_by_superseded: inverse_table(graph.sources, "supersedes"),
            attribution: HashMap::new(),
        };
        index.attribution = index.attribute_requirements();
        index
    }

    /// The record positions carrying `id` under `node_type`, in record
    /// order. Empty when no record of that kind holds the id.
    pub(super) fn records_with_id(&self, node_type: NodeType, id: &str) -> &[usize] {
        self.ids.bucket(node_type, id).map_or(&[], Vec::as_slice)
    }

    /// The positions of the rules a requirement produces: named in its
    /// `requirement_ids`, or named by a resolution that resolves it. One
    /// position per rule record, in rule record order.
    pub(super) fn produced_rule_indices(&self, requirement_id: &StableId) -> Vec<usize> {
        let mut positions: BTreeSet<usize> = self
            .rules_by_requirement
            .get(requirement_id.as_str())
            .into_iter()
            .flatten()
            .copied()
            .collect();
        for resolution in self
            .resolutions_by_requirement
            .get(requirement_id.as_str())
            .into_iter()
            .flatten()
        {
            let id = self.resolutions[*resolution].id.as_str();
            positions.extend(
                self.rules_by_resolution
                    .get(id)
                    .into_iter()
                    .flatten()
                    .copied(),
            );
        }
        positions.into_iter().collect()
    }

    /// The positions of the rules a resolution produces, in rule record
    /// order.
    pub(super) fn rules_produced_by(&self, resolution_id: &StableId) -> &[usize] {
        self.rules_by_resolution
            .get(resolution_id.as_str())
            .map_or(&[], Vec::as_slice)
    }

    /// The positions of the resolutions that resolve the requirement, in
    /// resolution record order.
    pub(super) fn resolving_resolution_indices(&self, requirement_id: &StableId) -> &[usize] {
        self.resolutions_by_requirement
            .get(requirement_id.as_str())
            .map_or(&[], Vec::as_slice)
    }

    /// The positions of the requirements refining the parent, in
    /// requirement record order.
    pub(super) fn child_requirement_indices(&self, parent_id: &StableId) -> &[usize] {
        self.children_by_parent
            .get(parent_id.as_str())
            .map_or(&[], Vec::as_slice)
    }

    /// The positions of the requirements whose `supersedes` names the
    /// target, in requirement record order.
    pub(super) fn superseding_requirement_indices(&self, target: &StableId) -> &[usize] {
        self.requirements_by_superseded
            .get(target.as_str())
            .map_or(&[], Vec::as_slice)
    }

    /// The positions of the resolutions whose `supersedes` names the
    /// target, in resolution record order.
    pub(super) fn superseding_resolution_indices(&self, target: &StableId) -> &[usize] {
        self.resolutions_by_superseded
            .get(target.as_str())
            .map_or(&[], Vec::as_slice)
    }

    /// The positions of the sources whose `supersedes` names the target,
    /// in source record order.
    pub(super) fn superseding_source_indices(&self, target: &StableId) -> &[usize] {
        self.sources_by_superseded
            .get(target.as_str())
            .map_or(&[], Vec::as_slice)
    }

    /// The positions of the requirements whose `spawned_by` names the
    /// resolution, in requirement record order.
    pub(super) fn spawned_requirement_indices(&self, resolution_id: &StableId) -> &[usize] {
        self.requirements_by_spawn
            .get(resolution_id.as_str())
            .map_or(&[], Vec::as_slice)
    }

    /// The positions of the requirements citing the source, in
    /// requirement record order.
    pub(super) fn citing_requirement_indices(&self, source_id: &StableId) -> &[usize] {
        self.requirements_by_source
            .get(source_id.as_str())
            .map_or(&[], Vec::as_slice)
    }

    /// The positions of the requirements whose id is among `entries`, in
    /// requirement record order. Entries naming a missing record
    /// contribute nothing; an entry two records carry contributes both.
    pub(super) fn requirements_named_by(
        &self,
        entries: impl Iterator<Item = &'graph StableId>,
    ) -> Vec<usize> {
        resolve_positions(entries, &self.ids.requirements)
    }

    /// The positions of the resolutions whose id is among `entries`, in
    /// resolution record order.
    pub(super) fn resolutions_named_by(
        &self,
        entries: impl Iterator<Item = &'graph StableId>,
    ) -> Vec<usize> {
        resolve_positions(entries, &self.ids.resolutions)
    }

    /// True when the source id is held by any record, which is what a
    /// valid citation needs.
    pub(super) fn source_held(&self, source_id: &StableId) -> bool {
        self.ids.sources.contains_key(source_id.as_str())
    }

    /// The requirements each rule answers to, read off one forward walk:
    /// every requirement in record order, attributed to each rule it
    /// produces, in rule record order.
    fn attribute_requirements(&self) -> HashMap<&'graph str, Vec<&'graph Requirement>> {
        let mut attribution: HashMap<&'graph str, Vec<&'graph Requirement>> = HashMap::new();
        for requirement in self.requirements {
            for position in self.produced_rule_indices(&requirement.id) {
                let attributed = attribution
                    .entry(self.rules[position].id.as_str())
                    .or_default();
                // Two rule records sharing an id would otherwise list the
                // same requirement twice; the outer loop visits each
                // requirement once, so checking the tail is enough.
                if attributed
                    .last()
                    .is_none_or(|last| last.id != requirement.id)
                {
                    attributed.push(requirement);
                }
            }
        }
        attribution
    }

    /// The requirements attributed to the rule, in the order the forward
    /// walk named them.
    pub(super) fn requirements_behind_rule(&self, rule_id: &StableId) -> &[&'graph Requirement] {
        self.attribution
            .get(rule_id.as_str())
            .map_or(&[], Vec::as_slice)
    }
}

/// One id table per node kind.
struct IdTables<'graph> {
    sources: HashMap<&'graph str, Vec<usize>>,
    requirements: HashMap<&'graph str, Vec<usize>>,
    resolutions: HashMap<&'graph str, Vec<usize>>,
    rules: HashMap<&'graph str, Vec<usize>>,
    topics: HashMap<&'graph str, Vec<usize>>,
    questions: HashMap<&'graph str, Vec<usize>>,
    domains: HashMap<&'graph str, Vec<usize>>,
    boundaries: HashMap<&'graph str, Vec<usize>>,
}

impl<'graph> IdTables<'graph> {
    fn new(graph: &GapGraph<'graph>) -> Self {
        Self {
            sources: id_table(graph.sources, |record| &record.id),
            requirements: id_table(graph.requirements, |record| &record.id),
            resolutions: id_table(graph.resolutions, |record| &record.id),
            rules: id_table(graph.rules, |record| &record.id),
            topics: id_table(graph.topics, |record| &record.id),
            questions: id_table(graph.questions, |record| &record.id),
            domains: id_table(graph.domains, |record| &record.id),
            boundaries: id_table(graph.boundaries, |record| &record.id),
        }
    }

    fn bucket(&self, node_type: NodeType, id: &str) -> Option<&Vec<usize>> {
        match node_type {
            NodeType::Source => self.sources.get(id),
            NodeType::Requirement => self.requirements.get(id),
            NodeType::Resolution => self.resolutions.get(id),
            NodeType::Rule => self.rules.get(id),
            NodeType::Topic => self.topics.get(id),
            NodeType::Question => self.questions.get(id),
            NodeType::Domain => self.domains.get(id),
            NodeType::Boundary => self.boundaries.get(id),
        }
    }
}

/// Maps each record's stable id to its position, one entry per record,
/// in record order.
fn id_table<'graph, T>(
    records: &'graph [T],
    key: impl Fn(&T) -> &StableId,
) -> HashMap<&'graph str, Vec<usize>> {
    let mut table: HashMap<&'graph str, Vec<usize>> = HashMap::new();
    for (position, record) in records.iter().enumerate() {
        table
            .entry(key(record).as_str())
            .or_default()
            .push(position);
    }
    table
}

/// Maps each target of one declared reference field to the positions of
/// the records naming it, in record order. A record naming the same
/// target twice enters its position once, the way a filter over the
/// record vector answers once per record.
fn inverse_table<'graph, T: RelationOwner>(
    records: &'graph [T],
    field: &str,
) -> HashMap<&'graph str, Vec<usize>> {
    let mut table: HashMap<&'graph str, Vec<usize>> = HashMap::new();
    for (position, record) in records.iter().enumerate() {
        let mut named: BTreeSet<&str> = BTreeSet::new();
        for (name, target) in record.references() {
            if name != field || !named.insert(target.as_str()) {
                continue;
            }
            table.entry(target.as_str()).or_default().push(position);
        }
    }
    table
}

/// Resolves reference entries to record positions in target record
/// order. Duplicate entries deduplicate; a target two records carry
/// answers with both positions.
fn resolve_positions<'graph>(
    entries: impl Iterator<Item = &'graph StableId>,
    table: &HashMap<&'graph str, Vec<usize>>,
) -> Vec<usize> {
    let mut positions: BTreeSet<usize> = BTreeSet::new();
    for target in entries {
        positions.extend(table.get(target.as_str()).into_iter().flatten().copied());
    }
    positions.into_iter().collect()
}
