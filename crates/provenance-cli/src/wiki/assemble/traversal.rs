use crate::wiki::model::{LineageEntry, PageLink};
use provenance_core::{Requirement, Resolution, Rule, StableId};
use std::collections::{BTreeMap, BTreeSet};

use super::context::Assembler;
use super::page_links::requirement_link;

impl<'a> Assembler<'a> {
    /// The requirement this one refines, as its `refines` field names it.
    pub(super) fn parent_id_of(&self, requirement_id: &StableId) -> Option<&'a StableId> {
        self.find_requirement(requirement_id)
            .and_then(|requirement| requirement.refines.as_ref())
    }

    pub(super) fn parent_of(&self, requirement_id: &StableId) -> Option<&'a Requirement> {
        self.parent_id_of(requirement_id)
            .and_then(|id| self.find_requirement(id))
    }

    /// The requirements whose `refines` names this one, in record order.
    pub(super) fn children_of(&self, requirement_id: &StableId) -> Vec<&'a Requirement> {
        self.state
            .requirements
            .iter()
            .filter(|child| child.refines.as_ref() == Some(requirement_id))
            .collect()
    }

    pub(super) fn resolving_resolutions(&self, requirement_id: &StableId) -> Vec<&'a Resolution> {
        self.query.resolving_resolutions(requirement_id)
    }

    pub(super) fn produced_rules_for_requirement(
        &self,
        requirement_id: &StableId,
    ) -> Vec<&'a Rule> {
        self.query.produced_rules_for_requirement(requirement_id)
    }

    pub(super) fn produced_rules_for_resolution(&self, resolution_id: &StableId) -> Vec<&'a Rule> {
        self.query.produced_rules_for_resolution(resolution_id)
    }

    /// The requirements a rule answers to, in record order.
    ///
    /// This is the inverse of [`Self::produced_rules_for_requirement`], read
    /// off one pass of that forward traversal rather than scanning the rule
    /// and resolution lists backwards a second time. A requirement page
    /// listing a rule and that rule's page listing the requirement are then
    /// the same fact, not two facts that happen to agree.
    pub(super) fn requirements_behind_rule(&self, rule_id: &StableId) -> &[&'a Requirement] {
        self.rule_requirements
            .get_or_init(|| {
                let mut attribution: BTreeMap<&'a str, Vec<&'a Requirement>> = BTreeMap::new();
                for requirement in &self.state.requirements {
                    for rule in self.produced_rules_for_requirement(&requirement.id) {
                        let attributed = attribution.entry(rule.id.as_str()).or_default();
                        // Two rule records sharing an id would otherwise list
                        // the same requirement twice; the outer loop visits
                        // each requirement once, so checking the tail is enough.
                        if attributed
                            .last()
                            .is_none_or(|last| last.id != requirement.id)
                        {
                            attributed.push(requirement);
                        }
                    }
                }
                attribution
            })
            .get(rule_id.as_str())
            .map_or(&[], Vec::as_slice)
    }

    /// The other requirements refining the same parent, in record order.
    pub(super) fn sibling_requirements(&self, requirement_id: &StableId) -> Vec<PageLink> {
        let Some(parent_id) = self.parent_id_of(requirement_id) else {
            return Vec::new();
        };
        self.children_of(parent_id)
            .into_iter()
            .filter(|candidate| candidate.id != *requirement_id)
            .map(requirement_link)
            .collect()
    }

    pub(super) fn lineage(&self, requirement: &'a Requirement) -> Vec<LineageEntry> {
        let mut chain = vec![requirement];
        let mut visited: BTreeSet<&str> = BTreeSet::from([requirement.id.as_str()]);
        let mut current = requirement;
        while let Some(parent) = self.parent_of(&current.id) {
            if !visited.insert(parent.id.as_str()) {
                break;
            }
            chain.push(parent);
            current = parent;
        }
        chain.reverse();
        let last = chain.len() - 1;
        chain
            .into_iter()
            .enumerate()
            .map(|(index, entry)| LineageEntry {
                link: requirement_link(entry),
                is_current: index == last,
            })
            .collect()
    }
}
