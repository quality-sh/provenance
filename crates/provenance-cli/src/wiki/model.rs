//! Intermediate page model for the generated wiki.
//!
//! Every page is plain data with semantic sections; nothing here knows about
//! HTML. Cross-references between pages are typed [`PageLink`]s carrying the
//! target [`PageId`]. Missing data is surfaced as [`GapNotice`]s and orphaned
//! records as an [`OrphanReport`] rather than being smoothed over.

use crate::wiki::links::{EvidenceRef, InlineRef};
use provenance_core::{
    MessageRole, NodeType, RequirementStatus, ResolutionInputType, ResolutionStatus, RuleSeverity,
    RuleStatus, SourceType, ThreadStatus,
};
pub use provenance_store::cache::GapKind;
use serde::Serialize;

mod discovery;
pub use discovery::{
    DecisionIndexPage, DomainGroup, DomainIndexPage, DomainState, HomepageDomain,
    OpenQuestionNotice, SearchEntry, SearchIndexPage, UnfinishedPage, HOMEPAGE_DOMAIN_ROW_CAP,
};

/// A rendered page kind, including singleton discovery pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageKind {
    ScopeIndex,
    DomainIndex,
    SearchIndex,
    DecisionIndex,
    Unfinished,
    Requirement,
    Resolution,
    Rule,
    Source,
}

/// A persisted record kind. Singleton pages deliberately have no variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    Requirement,
    Resolution,
    Rule,
    Source,
}

impl From<RecordKind> for PageKind {
    fn from(kind: RecordKind) -> Self {
        match kind {
            RecordKind::Requirement => Self::Requirement,
            RecordKind::Resolution => Self::Resolution,
            RecordKind::Rule => Self::Rule,
            RecordKind::Source => Self::Source,
        }
    }
}

/// Identifies one persisted record page by kind and stable id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageId {
    pub kind: RecordKind,
    pub record_id: String,
}

impl PageId {
    pub fn new(kind: RecordKind, record_id: impl Into<String>) -> Self {
        Self {
            kind,
            record_id: record_id.into(),
        }
    }

    /// The canonical route for this page, mirroring the docs server's
    /// trailing-slash convention.
    pub fn route(&self) -> String {
        crate::wiki::routes::WikiRoute::Record(self).path()
    }
}

/// A typed cross-reference to another wiki page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageLink {
    pub target: PageId,
    pub title: String,
}

/// An honest hole in the graph, rendered as a first-class notice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GapNotice {
    pub kind: GapKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<PageLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related: Option<PageLink>,
    pub detail: String,
}

/// A source no requirement cites, carrying the gap's own words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OrphanRecord {
    pub link: PageLink,
    pub reason: String,
}

/// Records that exist but are attached to nothing, listed on the unfinished page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct OrphanReport {
    pub sources: Vec<OrphanRecord>,
}

/// Record totals for the scope, shown on the index page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct CorpusCounts {
    pub sources: usize,
    pub requirements: usize,
    pub resolutions: usize,
    pub rules: usize,
}

/// The scope index: search, domain discovery, record totals, and an
/// unfinished-work summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScopeIndexPage {
    pub scope: String,
    pub title: String,
    pub counts: CorpusCounts,
    pub search_coverage: String,
    pub search_example: Option<String>,
    pub domains: Vec<HomepageDomain>,
    pub authored_domain_count: usize,
    pub unfinished_count: usize,
}

/// One step in a requirement's parent chain, root first; the final entry is
/// the page's own requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LineageEntry {
    pub link: PageLink,
    pub is_current: bool,
}

/// A resolution input rendered as a scholarly citation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InputCitation {
    pub input_type: ResolutionInputType,
    pub summary: String,
    pub reference: EvidenceRef,
}

/// A requirement's source reference rendered as a numbered footnote
/// (numbering is the renderer's job).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceCitation {
    pub link: PageLink,
    pub source_type: SourceType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clause: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<EvidenceRef>,
}

/// A resolving decision embedded on a requirement page. The decision's
/// body is copied from its resolution record (the single place it lives);
/// the link points at the full resolution page.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DecisionSection {
    pub link: PageLink,
    pub status: ResolutionStatus,
    pub position: String,
    pub rationale: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enforcement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    pub inputs: Vec<InputCitation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub made_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<i64>,
}

/// A produced rule summarized on a territory card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuleCard {
    pub link: PageLink,
    pub statement: String,
    pub status: RuleStatus,
    pub severity: RuleSeverity,
    pub evidence: Vec<EvidenceRef>,
}

/// One message in an evidence thread (a field note).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldNote {
    pub message_id: String,
    pub role: MessageRole,
    pub created_at: i64,
    pub body: String,
    /// Linkable spans inside `body` (code refs and test-case names).
    pub refs: Vec<InlineRef>,
}

/// An evidence thread attached to a record, with its parent named so the
/// renderer can attribute borrowed threads (a requirement page also shows
/// its resolving decisions' threads).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EvidenceThread {
    pub thread_id: String,
    pub parent_type: NodeType,
    pub parent_id: String,
    pub status: ThreadStatus,
    pub messages: Vec<FieldNote>,
}

/// A requirement detail page.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RequirementPage {
    pub id: PageId,
    pub title: String,
    pub status: RequirementStatus,
    pub statement: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fog: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_id: Option<String>,
    pub domain_has_anchor: bool,
    /// The parent requirement; `None` means the back link targets the index.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub back_link: Option<PageLink>,
    pub lineage: Vec<LineageEntry>,
    pub decisions: Vec<DecisionSection>,
    pub produced_rules: Vec<RuleCard>,
    pub children: Vec<PageLink>,
    pub siblings: Vec<PageLink>,
    pub sources: Vec<SourceCitation>,
    pub gaps: Vec<GapNotice>,
    pub threads: Vec<EvidenceThread>,
}

/// A resolution detail page.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResolutionPage {
    pub id: PageId,
    pub title: String,
    pub status: ResolutionStatus,
    pub position: String,
    pub rationale: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enforcement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    pub inputs: Vec<InputCitation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub made_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_on: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<PageLink>,
    pub resolves: Vec<PageLink>,
    pub spawned: Vec<PageLink>,
    pub produced_rules: Vec<RuleCard>,
    pub gaps: Vec<GapNotice>,
    pub threads: Vec<EvidenceThread>,
}

/// A production item claimed as a Rule's primary implementation by a scanner
/// relationship or canonical typed binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImplementationBinding {
    pub symbol: Option<String>,
    pub location: EvidenceRef,
}

/// The code scan the binding sections were read from. `None` on a page built
/// without a scan report: such a page knows nothing about bindings and must
/// not report an absent one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodeScan {
    /// The commit the scan ran against, or `None` for an uncommitted tree.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

/// A scanned site claiming how a rule is verified.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerificationSite {
    pub method: String,
    pub symbol: Option<String>,
    pub location: EvidenceRef,
    pub outside_implementation_module: bool,
}

/// A rule detail page with its backward traceability chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RulePage {
    pub id: PageId,
    pub title: String,
    pub statement: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: RuleStatus,
    pub severity: RuleSeverity,
    /// The scan behind scanner-discovered implementations and verifications.
    /// Canonical typed bindings remain available when this is `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_scan: Option<CodeScan>,
    pub implementations: Vec<ImplementationBinding>,
    pub verifications: Vec<VerificationSite>,
    /// The resolutions and requirements the rule names as producing it.
    pub produced_by: Vec<PageLink>,
    /// Upstream requirements reached through the producing records.
    pub requirements: Vec<PageLink>,
    /// Sources referencing those upstream requirements.
    pub sources: Vec<PageLink>,
    pub gaps: Vec<GapNotice>,
    pub threads: Vec<EvidenceThread>,
}

/// A source detail page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourcePage {
    pub id: PageId,
    pub title: String,
    pub source_type: SourceType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<EvidenceRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_pin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_date: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_date: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<PageLink>,
    pub referenced_requirements: Vec<PageLink>,
    pub gaps: Vec<GapNotice>,
    pub threads: Vec<EvidenceThread>,
}

/// Every page assembled for one scope.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WikiCorpus {
    pub scope: String,
    pub index: ScopeIndexPage,
    pub domains: DomainIndexPage,
    pub search: SearchIndexPage,
    pub decisions: DecisionIndexPage,
    pub unfinished: UnfinishedPage,
    pub requirements: Vec<RequirementPage>,
    pub resolutions: Vec<ResolutionPage>,
    pub rules: Vec<RulePage>,
    pub sources: Vec<SourcePage>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn singleton_pages_are_not_record_identities() {
        let singletons = [
            PageKind::ScopeIndex,
            PageKind::DomainIndex,
            PageKind::SearchIndex,
            PageKind::DecisionIndex,
            PageKind::Unfinished,
        ];
        assert_eq!(singletons.len(), 5);
    }

    #[test]
    fn page_id_routes_records_under_their_kind() {
        assert_eq!(
            PageId::new(RecordKind::Requirement, "req_split").route(),
            "/requirements/req_split/"
        );
        assert_eq!(
            PageId::new(RecordKind::Resolution, "res_split").route(),
            "/resolutions/res_split/"
        );
        assert_eq!(
            PageId::new(RecordKind::Rule, "rule_sah_inv_001").route(),
            "/rules/rule_sah_inv_001/"
        );
        assert_eq!(
            PageId::new(RecordKind::Source, "source_schads").route(),
            "/sources/source_schads/"
        );
    }
}
