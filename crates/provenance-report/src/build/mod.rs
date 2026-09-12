//! The envelope builder: the producer of schema-1 report envelopes from
//! live Provenance facts.
//!
//! Layers stay separate. This module reads a real repository at a named
//! base and head, gathers what the scan, the committed graph and the
//! evidence diff already compute, and writes an envelope. It renders
//! nothing, talks to no network, and calls no model. It never fabricates a
//! verification run, a pass, or a commit: `verification_runs` stays empty,
//! and the renderer states "not supplied" for what is absent. When the scan
//! cannot pin to the head commit, or the base graph cannot be compared, the
//! envelope says incomplete or incompatible and states the reason.

use crate::envelope::{
    BaselineCompatibility, Completeness, PolicyMode, PolicyResult, ReportEnvelope, ScanFacts,
    SUPPORTED_SCHEMA_VERSION,
};
use crate::render;
use anyhow::Context;
use camino::Utf8Path;
use provenance_core::ScopeId;
use provenance_macros::rule;
use provenance_store::stale::git;
use provenance_store::{cache, layout::ProvenanceLayout, settings};

mod findings;
mod graph_snapshots;

use findings::BaselineView;
use graph_snapshots::{GraphSnapshot, SnapshotRead};

/// Everything the builder needs: a repository, two commits in it, the scope
/// to read, and the repository identity the envelope carries.
pub struct BuildInput<'a> {
    /// The repository root that holds `.provenance` and the Git history.
    pub repo: &'a Utf8Path,
    /// The path the source scan walks; the repository root for a full scan.
    pub scan_path: &'a Utf8Path,
    pub scope: &'a str,
    /// Older endpoint of the comparison range.
    pub base: &'a str,
    /// Newer endpoint of the comparison range.
    pub head: &'a str,
    /// Repository identity in `owner/name` form.
    pub repository: &'a str,
}

/// Build one schema-1 envelope from the repository state.
///
/// The same state at the same two commits produces a byte-identical
/// envelope: the renderer's canonical normalization sorts every emitted
/// collection before return.
#[rule("rule_report_envelope_states_only_known_facts")]
pub fn build_envelope(input: &BuildInput<'_>) -> anyhow::Result<ReportEnvelope> {
    let scope = ScopeId::new(input.scope)?;
    let (base, head) = git::resolve_range(
        input.repo,
        Some(input.base.to_string()),
        Some(input.head.to_string()),
        None,
    )?;
    let layout = ProvenanceLayout::new(input.repo.to_path_buf());
    let configured = settings::Settings::load(&layout)
        .context("read repository settings for the policy outcome")?
        .coverage
        .binding_findings;

    let scans = provenance_scanner::scan_path_with_content(input.scan_path)?;
    let files_scanned = scans.len() as u64;
    let scans: Vec<_> = scans.into_iter().map(|file| file.scan).collect();
    let scan_covers = scan_covers_repository(input.repo, input.scan_path);
    let (completeness, incompleteness_reason) =
        scan_completeness(input.repo, input.scan_path, scan_covers, &head)?;
    let complete_scan = completeness == Completeness::Complete;

    let (baseline, baseline_reason, base_snapshot, head_snapshot) =
        read_snapshots(input.repo, &base, &head, &scope)?;
    let (verifications, implementations) =
        graph_snapshots::read_bindings(input.repo, &head, &scope)?;

    let finding_records = collect_findings(&FindingInput {
        repo: input.repo,
        base: &base,
        head: &head,
        scope: &scope,
        layout: &layout,
        scans: &scans,
        verifications: &verifications,
        implementations: &implementations,
        base_snapshot: &base_snapshot,
        head_snapshot: &head_snapshot,
        baseline,
        baseline_view: &BaselineView::for_rules(baseline, &base_snapshot.rules),
        binding_severity: binding_severity(configured),
        complete_scan,
    })?;
    let governed = finding_records
        .iter()
        .filter(|finding| {
            finding.code == "active_rule_missing_verification"
                || finding.code == "inactive_rule_current_binding"
        })
        .count();
    let graph_changes = if baseline == BaselineCompatibility::Compatible {
        graph_snapshots::diff_snapshots(&base_snapshot, &head_snapshot)
    } else {
        Vec::new()
    };

    let envelope = ReportEnvelope {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        repository: input.repository.to_string(),
        scope: input.scope.to_string(),
        base_commit: base,
        head_commit: head,
        scan: ScanFacts {
            completeness,
            incompleteness_reason,
            baseline,
            baseline_reason,
            files_scanned,
            failure: None,
        },
        policy: Some(crate::envelope::PolicyOutcome {
            mode: policy_mode(configured),
            result: policy_result(configured, governed),
        }),
        graph_changes,
        findings: finding_records,
        // Verification-run facts are out of scope for this producer. A run
        // the layers cannot supply stays absent; it is never invented.
        verification_runs: Vec::new(),
    };
    Ok(render::normalize(&envelope))
}

/// Read both committed snapshots. The head must parse: the report describes
/// the head state, and an unreadable head is an operation error. The base
/// read produces a compatibility verdict instead.
type SnapshotPair = (
    BaselineCompatibility,
    Option<String>,
    GraphSnapshot,
    GraphSnapshot,
);

fn read_snapshots(
    repo: &Utf8Path,
    base: &str,
    head: &str,
    scope: &ScopeId,
) -> anyhow::Result<SnapshotPair> {
    let head_snapshot = match graph_snapshots::read_snapshot(repo, head, scope) {
        SnapshotRead::Present(snapshot) => snapshot,
        SnapshotRead::Absent => GraphSnapshot::default(),
        SnapshotRead::Incompatible(reason) => {
            anyhow::bail!("graph records at head commit {head} do not parse: {reason}")
        }
    };
    let (baseline, baseline_reason, base_snapshot) =
        match graph_snapshots::read_snapshot(repo, base, scope) {
            SnapshotRead::Present(snapshot) => (BaselineCompatibility::Compatible, None, snapshot),
            SnapshotRead::Absent => (
                BaselineCompatibility::Missing,
                Some(format!(
                    "no Provenance graph records exist at base commit {base}"
                )),
                GraphSnapshot::default(),
            ),
            SnapshotRead::Incompatible(reason) => (
                BaselineCompatibility::Incompatible,
                Some(format!(
                    "graph records at base commit {base} do not parse: {reason}"
                )),
                GraphSnapshot::default(),
            ),
        };
    Ok((baseline, baseline_reason, base_snapshot, head_snapshot))
}

/// Gather every finding the layers support under the current scan and
/// baseline facts. Absence needs a complete scan: an incomplete scan cannot
/// clear an absence, so it never reports one. Comparative findings need a
/// compatible baseline.
fn collect_findings(input: &FindingInput<'_>) -> anyhow::Result<Vec<crate::envelope::Finding>> {
    let rules = &input.head_snapshot.rules;
    let mut finding_records = Vec::new();
    if input.complete_scan {
        finding_records.extend(findings::absence_findings(
            rules,
            input.scans,
            input.verifications,
            input.baseline_view,
            input.binding_severity,
        ));
    }
    finding_records.extend(findings::inactive_current_findings(
        rules,
        input.scans,
        input.implementations,
        input.verifications,
        input.baseline_view,
        input.binding_severity,
        input.repo,
    ));
    if input.baseline == BaselineCompatibility::Compatible {
        finding_records.extend(findings::statement_change_findings(
            &input.base_snapshot.requirements,
            &input.head_snapshot.requirements,
            &input.base_snapshot.rules,
            &input.head_snapshot.rules,
        ));
        if input.complete_scan {
            finding_records.extend(evidence_site_findings(
                input.repo,
                input.base,
                input.head,
                input.scope,
                input.layout,
            )?);
        }
    }
    Ok(finding_records)
}

/// The gathered facts one finding pass reads.
struct FindingInput<'a> {
    repo: &'a Utf8Path,
    base: &'a str,
    head: &'a str,
    scope: &'a ScopeId,
    layout: &'a ProvenanceLayout,
    scans: &'a [provenance_scanner::FileScan],
    verifications: &'a [provenance_core::VerificationBinding],
    implementations: &'a [provenance_core::ImplementationBinding],
    base_snapshot: &'a GraphSnapshot,
    head_snapshot: &'a GraphSnapshot,
    baseline: BaselineCompatibility,
    baseline_view: &'a BaselineView,
    binding_severity: crate::envelope::Severity,
    complete_scan: bool,
}

const fn binding_severity(
    configured: settings::BindingFindingsSeverity,
) -> crate::envelope::Severity {
    match configured {
        settings::BindingFindingsSeverity::Warning => crate::envelope::Severity::Warning,
        settings::BindingFindingsSeverity::Error => crate::envelope::Severity::Error,
    }
}

const fn policy_mode(configured: settings::BindingFindingsSeverity) -> PolicyMode {
    match configured {
        settings::BindingFindingsSeverity::Warning => PolicyMode::Warning,
        settings::BindingFindingsSeverity::Error => PolicyMode::Error,
    }
}

const fn policy_result(
    configured: settings::BindingFindingsSeverity,
    governed: usize,
) -> PolicyResult {
    if matches!(configured, settings::BindingFindingsSeverity::Error) && governed > 0 {
        PolicyResult::Failure
    } else {
        PolicyResult::Success
    }
}

/// The evidence-diff facts for a complete scan over a compatible baseline.
fn evidence_site_findings(
    repo: &Utf8Path,
    base: &str,
    head: &str,
    scope: &ScopeId,
    layout: &ProvenanceLayout,
) -> anyhow::Result<Vec<crate::envelope::Finding>> {
    let graph = cache::graph_evidence(layout, scope, false)?;
    let base_files = git::revision_files(repo, base)?;
    let head_files = git::revision_files(repo, head)?;
    let changes = git::changed_files(repo, base, head)?;
    let report = provenance_store::stale::gate::report(
        repo,
        base.to_string(),
        head.to_string(),
        base_files,
        head_files,
        &changes,
        &graph,
    );
    Ok(findings::evidence_site_findings(&report))
}

/// Whether the scan covers the declared scope. A partial scan cannot claim
/// that a Rule has no implementation or verification elsewhere.
fn scan_covers_repository(repo: &Utf8Path, path: &Utf8Path) -> bool {
    same_file::is_same_file(repo, path).unwrap_or(false)
}

/// Whether the working tree matches the head commit. Scanned source facts
/// come from the working tree; only a clean tree pins them to head.
fn working_tree_is_clean(repo: &Utf8Path) -> anyhow::Result<bool> {
    let output = std::process::Command::new("git")
        .current_dir(repo)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "git status failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(output.stdout.is_empty())
}

fn scan_completeness(
    repo: &Utf8Path,
    scan_path: &Utf8Path,
    scan_covers: bool,
    head: &str,
) -> anyhow::Result<(Completeness, Option<String>)> {
    let clean = scan_covers && working_tree_is_clean(repo)?;
    if clean {
        return Ok((Completeness::Complete, None));
    }
    let reason = if scan_covers {
        format!(
            "the working tree has uncommitted changes; scanned source facts do not \
             pin to head commit {head}"
        )
    } else {
        format!(
            "the scan path {scan_path} covers part of the repository tree; absence \
             facts are not established for the whole tree"
        )
    };
    Ok((Completeness::Incomplete, Some(reason)))
}
