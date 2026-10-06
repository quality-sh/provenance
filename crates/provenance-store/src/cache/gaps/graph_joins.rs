//! The relationship lookups over [`GraphQuery`] and its index.
//!
//! Every join answers in the record order of the vectors behind the
//! graph, so output stays deterministic for one loaded export. The
//! inverse joins (who names this id?) read their bucket in owner record
//! order; the forward joins (which records does this record name?) sort
//! the candidates they collect, so they read in target record order.

use super::graph_query::GraphQuery;
use provenance_core::{NodeType, Requirement, Resolution, Rule, Source, StableId};

impl<'graph> GraphQuery<'_, 'graph> {
    /// The resolutions whose `requirement_ids` name the requirement, in
    /// resolution record order.
    pub fn resolving_resolutions(&self, requirement_id: &StableId) -> Vec<&'graph Resolution> {
        self.index
            .resolving_resolution_indices(requirement_id)
            .iter()
            .map(|position| &self.graph.resolutions[*position])
            .collect()
    }

    /// The rules a requirement produces: named in `requirement_ids`, or
    /// named in `resolution_ids` by a resolution that resolves it. One
    /// row per rule record, in rule record order.
    pub fn produced_rules_for_requirement(&self, requirement_id: &StableId) -> Vec<&'graph Rule> {
        self.index
            .produced_rule_indices(requirement_id)
            .into_iter()
            .map(|position| &self.graph.rules[position])
            .collect()
    }

    /// The rules the resolution produces, in rule record order.
    pub fn produced_rules_for_resolution(&self, resolution_id: &StableId) -> Vec<&'graph Rule> {
        self.index
            .rules_produced_by(resolution_id)
            .iter()
            .map(|position| &self.graph.rules[*position])
            .collect()
    }

    /// The requirements a rule names, in requirement record order. A
    /// named requirement that is not in the scope is a dangling
    /// reference, not a producer.
    pub fn producing_requirements(&self, rule_id: &StableId) -> Vec<&'graph Requirement> {
        let Some(rule) = self.find_rule(rule_id) else {
            return Vec::new();
        };
        self.index
            .requirements_named_by(rule.requirement_ids.iter())
            .into_iter()
            .map(|position| &self.graph.requirements[position])
            .collect()
    }

    /// The resolutions a rule names, on the same terms as
    /// [`Self::producing_requirements`].
    pub fn producing_resolutions(&self, rule_id: &StableId) -> Vec<&'graph Resolution> {
        let Some(rule) = self.find_rule(rule_id) else {
            return Vec::new();
        };
        self.index
            .resolutions_named_by(rule.resolution_ids.iter())
            .into_iter()
            .map(|position| &self.graph.resolutions[position])
            .collect()
    }

    /// The requirements the resolution resolves, in requirement record
    /// order. A named requirement outside the scope resolves nothing.
    pub fn requirements_resolved_by(&self, resolution: &Resolution) -> Vec<&'graph Requirement> {
        self.index
            .requirements_named_by(resolution.requirement_ids.iter())
            .into_iter()
            .map(|position| &self.graph.requirements[position])
            .collect()
    }

    /// The requirements whose `spawned_by` names the resolution, in
    /// requirement record order. The join reads the field, so it answers
    /// by id whether or not the resolution itself is in scope.
    pub fn requirements_spawned_by(&self, resolution_id: &StableId) -> Vec<&'graph Requirement> {
        self.index
            .spawned_requirement_indices(resolution_id)
            .iter()
            .map(|position| &self.graph.requirements[*position])
            .collect()
    }

    /// The requirements citing the source under `source_refs`, in
    /// requirement record order.
    pub fn requirements_citing_source(&self, source_id: &StableId) -> Vec<&'graph Requirement> {
        self.index
            .citing_requirement_indices(source_id)
            .iter()
            .map(|position| &self.graph.requirements[*position])
            .collect()
    }

    /// The requirements refining the parent, in requirement record
    /// order. A dangling `refines` target has no children row.
    pub fn children_of(&self, requirement_id: &StableId) -> Vec<&'graph Requirement> {
        self.index
            .child_requirement_indices(requirement_id)
            .iter()
            .map(|position| &self.graph.requirements[*position])
            .collect()
    }

    /// The requirement this one refines, when the scope holds it.
    pub fn refines_parent(&self, requirement_id: &StableId) -> Option<&'graph Requirement> {
        let requirement = self.find_requirement(requirement_id)?;
        self.find_requirement(requirement.refines.as_ref()?)
    }

    /// The requirements the `supersedes` field of this record reaches,
    /// in requirement record order. Dangling entries reach nothing.
    pub fn requirement_supersedes(&self, requirement: &Requirement) -> Vec<&'graph Requirement> {
        self.index
            .requirements_named_by(requirement.supersedes.iter())
            .into_iter()
            .map(|position| &self.graph.requirements[position])
            .collect()
    }

    /// The requirements the `depends_on` field of this record reaches,
    /// on the same terms as [`Self::requirement_supersedes`].
    pub fn requirement_depends_on(&self, requirement: &Requirement) -> Vec<&'graph Requirement> {
        self.index
            .requirements_named_by(requirement.depends_on.iter())
            .into_iter()
            .map(|position| &self.graph.requirements[position])
            .collect()
    }

    /// The requirement whose `supersedes` names this one with the lowest
    /// id: the first in id order when several do.
    pub fn requirement_superseded_by(
        &self,
        requirement_id: &StableId,
    ) -> Option<&'graph Requirement> {
        self.index
            .superseding_requirement_indices(requirement_id)
            .iter()
            .map(|position| &self.graph.requirements[*position])
            .min_by_key(|candidate| candidate.id.as_str())
    }

    /// The resolution whose `supersedes` names this one with the lowest
    /// id.
    pub fn resolution_superseded_by(&self, resolution_id: &StableId) -> Option<&'graph Resolution> {
        self.index
            .superseding_resolution_indices(resolution_id)
            .iter()
            .map(|position| &self.graph.resolutions[*position])
            .min_by_key(|candidate| candidate.id.as_str())
    }

    /// The source whose `supersedes` names this one with the lowest id.
    pub fn source_superseded_by(&self, source_id: &StableId) -> Option<&'graph Source> {
        self.index
            .superseding_source_indices(source_id)
            .iter()
            .map(|position| &self.graph.sources[*position])
            .min_by_key(|candidate| candidate.id.as_str())
    }

    /// The requirements a rule answers to, in the order the forward
    /// traversal named them.
    ///
    /// This is the inverse of [`Self::produced_rules_for_requirement`],
    /// read off one pass of that forward traversal rather than scanning
    /// the rule and resolution lists backwards a second time. A
    /// requirement page listing a rule and that rule's page listing the
    /// requirement are then the same fact, not two facts that happen to
    /// agree.
    pub fn requirements_behind_rule(&self, rule_id: &StableId) -> &[&'graph Requirement] {
        self.index.requirements_behind_rule(rule_id)
    }

    /// True when a source reaches this rule through a requirement that
    /// produces it. A sourced requirement elsewhere in the scope says
    /// nothing about this rule.
    pub fn rule_trace_reaches_source(&self, rule_id: &StableId) -> bool {
        self.producing_requirements(rule_id)
            .into_iter()
            .any(|requirement| self.requirement_has_valid_source(requirement))
    }

    pub fn requirement_has_valid_source(&self, requirement: &Requirement) -> bool {
        requirement
            .source_refs
            .iter()
            .any(|reference| self.index.source_held(&reference.source_id))
    }

    pub fn source_is_referenced(&self, source_id: &StableId) -> bool {
        !self.index.citing_requirement_indices(source_id).is_empty()
    }

    /// The first rule record with the id, shared by the rule-side
    /// lookups.
    fn find_rule(&self, rule_id: &StableId) -> Option<&'graph Rule> {
        self.index
            .records_with_id(NodeType::Rule, rule_id.as_str())
            .first()
            .map(|position| &self.graph.rules[*position])
    }
}
