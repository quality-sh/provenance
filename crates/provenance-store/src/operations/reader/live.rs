//! The live parts of an answer: what the stamp does not cover.
//!
//! Each half is reachable only through a handle taken with
//! [`ReadContext::live`], which records the word, and each method exists
//! on its own word only: a handle asked for another word's read is a
//! programming error.

use super::ReadContext;
use crate::cache::{self, GraphEvidence};
use crate::layout::ProvenanceLayout;
use crate::stale::{gate, git};
use crate::state_store::StateStore;
use camino::Utf8Path;
use provenance_core::coverage::{EvidenceDiffSite, EvidenceDiffState};
use provenance_core::{ScopeId, VerificationRun};
use provenance_scanner::FileScan;

/// The closed list of live words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Live {
    /// Canonical shards under `.provenance/state`.
    Canonical,
    /// A git diff between two commits.
    Diff,
    /// A scan of the working tree.
    ScannedSites,
    /// Verification runs, cache JSONL outside the projection.
    VerificationRuns,
}

impl Live {
    pub const fn word(self) -> &'static str {
        match self {
            Self::Canonical => "canonical",
            Self::Diff => "diff",
            Self::ScannedSites => "scanned_sites",
            Self::VerificationRuns => "verification_runs",
        }
    }
}

pub struct LiveHandle<'c> {
    context: &'c ReadContext,
    what: Live,
}

impl<'c> LiveHandle<'c> {
    pub(super) const fn new(context: &'c ReadContext, what: Live) -> Self {
        Self { context, what }
    }

    fn only(&self, what: Live) {
        assert!(
            self.what == what,
            "a {} handle cannot read {}",
            self.what.word(),
            what.word()
        );
    }

    fn layout(&self) -> ProvenanceLayout {
        ProvenanceLayout::new(self.context.repo().to_path_buf())
    }

    /// The canonical store behind the repository.
    pub fn store(&self) -> StateStore {
        self.only(Live::Canonical);
        StateStore::new(self.layout())
    }

    /// The canonical graph references that name a repository path, which
    /// the diff half is read against.
    pub fn graph_evidence(
        &self,
        scope: &ScopeId,
        include_retired: bool,
    ) -> anyhow::Result<GraphEvidence> {
        self.only(Live::Canonical);
        cache::graph_evidence(&self.layout(), scope, include_retired)
    }

    /// A scan of the whole working tree. A test can set the scan in
    /// advance, so a timing row measures graph and binding work only.
    pub fn scan_tree(&self) -> anyhow::Result<Vec<FileScan>> {
        self.only(Live::ScannedSites);
        if let Some(scans) = crate::test_probes::test_scan() {
            return Ok(scans);
        }
        provenance_scanner::scan_path(self.context.repo())
    }

    /// The scope's verification runs.
    pub fn runs(&self, scope: &ScopeId) -> anyhow::Result<Vec<VerificationRun>> {
        self.only(Live::VerificationRuns);
        StateStore::new(self.layout()).list_verification_runs(scope)
    }

    /// The two commits a range names, resolved; `head` defaults to the
    /// current commit. A range that does not resolve refuses here, before
    /// anything else is read.
    pub fn resolve_range(
        &self,
        base: String,
        head: Option<String>,
    ) -> anyhow::Result<(String, String)> {
        self.only(Live::Diff);
        git::resolve_range(
            self.context.repo(),
            Some(base),
            Some(head.unwrap_or_else(|| "HEAD".to_string())),
            None,
        )
    }

    /// What the resolved commit range did to the code carrying graph
    /// evidence.
    pub fn disturbed(
        &self,
        base: String,
        head: String,
        rules: &[String],
        graph: &GraphEvidence,
    ) -> anyhow::Result<Disturbed> {
        self.only(Live::Diff);
        disturbed(self.context.repo(), base, head, rules, graph)
    }
}

/// What a commit range did to the code carrying graph evidence. Stale is
/// read from a diff, never guessed from the working tree.
pub struct Disturbed {
    pub base: String,
    pub head: String,
    pub files_changed: usize,
    pub sites: Vec<EvidenceDiffSite>,
}

pub fn disturbed(
    repo: &Utf8Path,
    base: String,
    head: String,
    rules: &[String],
    graph: &GraphEvidence,
) -> anyhow::Result<Disturbed> {
    let base_files = git::revision_files(repo, &base)?;
    let head_files = git::revision_files(repo, &head)?;
    let changes = git::changed_files(repo, &base, &head)?;
    let report = gate::report(repo, base, head, base_files, head_files, &changes, graph);
    let sites = report
        .sites
        .into_iter()
        .filter(|site| rules.is_empty() || rules.contains(&site.subject_id))
        .filter(|site| site.state != EvidenceDiffState::Untouched)
        .collect();
    Ok(Disturbed {
        base: report.base,
        head: report.head,
        files_changed: report.files_changed,
        sites,
    })
}
