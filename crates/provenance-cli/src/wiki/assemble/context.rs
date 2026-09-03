use crate::handlers::ScopeExport;
use crate::wiki::links::LinkResolver;
use provenance_core::coverage::CoverageReport;
use provenance_core::{Requirement, Source, StableId};
use provenance_store::cache::{GapItem, GraphQuery};
use std::cell::OnceCell;
use std::collections::BTreeMap;

pub(super) struct Assembler<'a> {
    pub(super) state: &'a ScopeExport,
    pub(super) resolver: &'a LinkResolver,
    pub(super) coverage: Option<&'a CoverageReport>,
    pub(super) gaps: &'a [GapItem],
    /// The same traversals gap policy runs on. The wiki reads decisions and
    /// produced rules through this rather than re-deriving them, so a page
    /// and its gap notices can never disagree about what the graph says.
    pub(super) query: GraphQuery<'a, 'a>,
    /// Rule id to the requirements that rule answers to, inverted from the
    /// forward traversal on first use. A rule page must not walk the graph
    /// backwards on its own: one walk, read in both directions.
    pub(super) rule_requirements: OnceCell<BTreeMap<&'a str, Vec<&'a Requirement>>>,
}

impl<'a> Assembler<'a> {
    pub(super) fn find_requirement(&self, id: &StableId) -> Option<&'a Requirement> {
        self.state
            .requirements
            .iter()
            .find(|requirement| requirement.id == *id)
    }

    pub(super) fn find_source(&self, id: &StableId) -> Option<&'a Source> {
        self.state.sources.iter().find(|source| source.id == *id)
    }
}
