use crate::handlers::ScopeExport;
use crate::wiki::links::LinkResolver;
use provenance_core::coverage::CoverageReport;
use provenance_store::cache::{GapItem, GraphQuery};

pub(super) struct Assembler<'a> {
    pub(super) state: &'a ScopeExport,
    pub(super) resolver: &'a LinkResolver,
    pub(super) coverage: Option<&'a CoverageReport>,
    pub(super) gaps: &'a [GapItem],
    /// Every record lookup, graph join, and inverse rule attribution the
    /// pages need. The same traversals gap policy runs on, built once
    /// over the loaded export, so a page and its gap notices can never
    /// disagree about what the graph says.
    pub(super) query: GraphQuery<'a, 'a>,
}
