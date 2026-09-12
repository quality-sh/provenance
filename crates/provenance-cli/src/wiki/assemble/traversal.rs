//! The `refines` walks the wiki renders as lineage and siblings.
//!
//! Every lookup is an indexed `GraphQuery` join; this module only adds
//! the walk state those joins do not carry: where the walk started, and
//! which records it has already visited, so a cycle in `refines` ends
//! the walk instead of hanging.

use crate::wiki::model::{LineageEntry, PageLink};
use provenance_core::{Requirement, StableId};

use super::context::Assembler;
use super::page_links::requirement_link;

impl<'a> Assembler<'a> {
    /// The other requirements refining the same parent, in record order.
    pub(super) fn sibling_requirements(&self, requirement_id: &StableId) -> Vec<PageLink> {
        let Some(parent) = self.query.refines_parent(requirement_id) else {
            return Vec::new();
        };
        self.query
            .children_of(&parent.id)
            .into_iter()
            .filter(|candidate| candidate.id != *requirement_id)
            .map(requirement_link)
            .collect()
    }

    pub(super) fn lineage(&self, requirement: &'a Requirement) -> Vec<LineageEntry> {
        let mut chain = vec![requirement];
        let mut visited: std::collections::BTreeSet<&str> =
            std::collections::BTreeSet::from([requirement.id.as_str()]);
        let mut current = requirement;
        while let Some(parent) = self.query.refines_parent(&current.id) {
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
