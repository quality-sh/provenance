use crate::cli::workspace::CoverageCommand;
use crate::output::{self, ReportFormat};
use crate::repo_context::RepoContext;
use anyhow::Context;
use camino::Utf8PathBuf;
use provenance_core::{Manifest, Scope};
use std::borrow::Cow;

mod render;
mod verification_state;
use render::render_coverage;
use verification_state::load_validation_state;

pub(super) struct CoverageScanOutcome {
    pub(super) scan: provenance_core::coverage::CoverageScan,
    pub(super) governed_finding_count: usize,
}

fn coverage_scan_against(
    repo: &camino::Utf8Path,
    path: &Utf8PathBuf,
    scope: &Scope,
    manifest: &Manifest,
    validate_rules: bool,
    baseline: Option<&camino::Utf8Path>,
) -> anyhow::Result<CoverageScanOutcome> {
    let scanned = provenance_scanner::scan_path_with_content(path)?;
    let selection = Selection { scope, manifest };
    coverage_scan_from_scanned_against(repo, path, &selection, validate_rules, &scanned, baseline)
}

pub(super) fn coverage_scan_from_scanned(
    repo: &camino::Utf8Path,
    path: &Utf8PathBuf,
    scope: &Scope,
    manifest: &Manifest,
    scanned: &[provenance_scanner::FileScanWithContent],
) -> anyhow::Result<CoverageScanOutcome> {
    let selection = Selection { scope, manifest };
    coverage_scan_from_scanned_against(repo, path, &selection, true, scanned, None)
}

/// The selected scope and the manifest that declares it. The manifest
/// decides which scanned files the scope owns.
struct Selection<'a> {
    scope: &'a Scope,
    manifest: &'a Manifest,
}

/// The scanned files that the scope owns: core's longest-prefix ownership
/// gives each file to the scope with the longest path prefix that contains
/// it, so an outer scope never sees the bindings of an inner scope. A file
/// outside the repository has no path prefix; a `.` scope keeps it.
fn scope_files<'a>(
    repo: &camino::Utf8Path,
    selection: &Selection<'_>,
    scanned: &'a [provenance_scanner::FileScanWithContent],
) -> Cow<'a, [provenance_scanner::FileScanWithContent]> {
    if let [only] = selection.manifest.scopes.as_slice() {
        if only.id == selection.scope.id && only.path_prefix.as_path() == camino::Utf8Path::new(".")
        {
            return Cow::Borrowed(scanned);
        }
    }
    let Ok(root) = std::fs::canonicalize(repo) else {
        return Cow::Owned(Vec::new());
    };
    let keeps_outside = selection.scope.path_prefix.as_path() == camino::Utf8Path::new(".");
    Cow::Owned(
        scanned
            .iter()
            .filter(|file| {
                repo_relative(&root, &file.scan.file_path).map_or(keeps_outside, |relative| {
                    provenance_core::scope::scope_owns_path(
                        selection.manifest,
                        &selection.scope.id,
                        &relative,
                    )
                })
            })
            .cloned()
            .collect(),
    )
}

/// The file's path inside the canonical repository root, or `None` when the
/// file resolves outside it.
fn repo_relative(root: &std::path::Path, file: &camino::Utf8Path) -> Option<Utf8PathBuf> {
    let resolved = std::fs::canonicalize(file).ok()?;
    Utf8PathBuf::from_path_buf(resolved.strip_prefix(root).ok()?.to_path_buf()).ok()
}

/// The warning for a scope whose path prefix names no directory, so a
/// mistyped prefix does not read as a scope with no bindings.
#[provenance_macros::rule("rule_coverage_names_missing_scope_directory")]
fn missing_scope_directory(
    repo: &camino::Utf8Path,
    scope: &Scope,
) -> Option<provenance_core::coverage::ValidationWarning> {
    let prefix = scope.path_prefix.as_path();
    (!repo.join(prefix).is_dir()).then(|| provenance_core::coverage::ValidationWarning {
        rule_id: String::new(),
        file_path: Some(prefix.to_path_buf()),
        line: None,
        message: format!(
            "scope `{}` has path prefix `{prefix}`, which names no directory in the repository",
            scope.id.as_str()
        ),
        binding_finding: false,
    })
}

/// Whether the scanned path contains every file of the scope, so an active
/// Rule with no binding in the scan is unimplemented rather than unseen.
fn scan_covers_scope(repo: &camino::Utf8Path, scope: &Scope, path: &camino::Utf8Path) -> bool {
    let Ok(root) = std::fs::canonicalize(repo.join(scope.path_prefix.as_path())) else {
        // A scope whose directory does not exist has no files to miss.
        return true;
    };
    std::fs::canonicalize(path).is_ok_and(|path| root.starts_with(path))
}

fn coverage_scan_from_scanned_against(
    repo: &camino::Utf8Path,
    path: &Utf8PathBuf,
    selection: &Selection<'_>,
    validate_rules: bool,
    scanned: &[provenance_scanner::FileScanWithContent],
    baseline: Option<&camino::Utf8Path>,
) -> anyhow::Result<CoverageScanOutcome> {
    let scope = selection.scope;
    let scanned = scope_files(repo, selection, scanned);
    let scanned = scanned.as_ref();
    let scans = scanned
        .iter()
        .map(|file| file.scan.clone())
        .collect::<Vec<_>>();
    let commit = scan_commit(repo, &scans);
    let validation = load_validation_state(repo, scope.id.as_str(), &scans, validate_rules)?;
    let mut warnings = validation.warnings;
    warnings.extend(missing_scope_directory(repo, scope));
    let mut governed_finding_count = 0;
    if validate_rules {
        let completeness = if scan_covers_scope(repo, scope, path) {
            provenance_scanner::RuleEvidenceCompleteness::Complete
        } else {
            provenance_scanner::RuleEvidenceCompleteness::Incomplete
        };
        let facts = provenance_scanner::derive_rule_evidence_facts(
            &validation.rules,
            &scans,
            &validation.implementations,
            &validation.bindings,
            completeness,
        );
        governed_finding_count = facts.governed_finding_count();
        warnings.extend(rule_evidence_warnings(facts));
    }
    let baseline_scan = if let Some(baseline) = baseline {
        let bytes = std::fs::read(baseline)
            .with_context(|| format!("read coverage baseline {baseline}"))?;
        Some(
            serde_json::from_slice(&bytes)
                .with_context(|| format!("parse coverage baseline {baseline}"))?,
        )
    } else {
        None
    };
    let baseline = baseline_scan
        .as_ref()
        .map(|scan| provenance_scanner::CoverageBaseline {
            scan,
            repo,
            scan_path: path,
            validate_rules,
        });
    let mut report = provenance_scanner::scan_to_coverage(scanned, commit, warnings, baseline);
    report.report.verification_bindings = validation.bindings;
    Ok(CoverageScanOutcome {
        scan: report.into_scan(),
        governed_finding_count,
    })
}

fn rule_evidence_warnings(
    facts: provenance_scanner::RuleEvidenceFacts,
) -> Vec<provenance_core::coverage::ValidationWarning> {
    let mut warnings = Vec::new();
    warnings.extend(facts.inactive_current.into_iter().map(|binding| {
        let status = match binding.status {
            provenance_core::RuleStatus::Deprecated => "deprecated",
            provenance_core::RuleStatus::Archived => "archived",
            _ => unreachable!("the derivation returns only inactive Rules"),
        };
        let kind = match binding.origin {
            provenance_scanner::InactiveBindingOrigin::Scanned => "marker",
            provenance_scanner::InactiveBindingOrigin::TypedImplementation => {
                "typed implementation binding"
            }
            provenance_scanner::InactiveBindingOrigin::TypedVerification => {
                "typed verification binding"
            }
        };
        provenance_core::coverage::ValidationWarning {
            message: format!(
                "{kind} cites rule `{}` with status `{status}`",
                binding.rule_id
            ),
            rule_id: binding.rule_id,
            file_path: Some(binding.file_path),
            line: binding.line,
            binding_finding: true,
        }
    }));
    warnings.extend(facts.unimplemented.into_iter().map(|rule_id| {
        provenance_core::coverage::ValidationWarning {
            message: format!("active rule `{rule_id}` has no implementation"),
            rule_id,
            file_path: None,
            line: None,
            binding_finding: false,
        }
    }));
    warnings.extend(facts.unverified.into_iter().map(|rule_id| {
        provenance_core::coverage::ValidationWarning {
            message: format!("active rule `{rule_id}` has no verification"),
            rule_id,
            file_path: None,
            line: None,
            binding_finding: true,
        }
    }));
    warnings
}

pub(super) fn current_git_commit(repo: &camino::Utf8Path) -> anyhow::Result<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo)
        .output()?;
    anyhow::ensure!(output.status.success(), "git rev-parse failed");
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn scan_commit(repo: &camino::Utf8Path, scans: &[provenance_scanner::FileScan]) -> Option<String> {
    if scans.is_empty() {
        return current_git_commit(repo).ok();
    }
    let mut command = std::process::Command::new("git");
    command
        .args([
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--ignored=matching",
            "--",
        ])
        .current_dir(repo);
    for scan in scans {
        let path = scan
            .file_path
            .strip_prefix(repo)
            .or_else(|_| scan.file_path.strip_prefix("."))
            .unwrap_or(&scan.file_path);
        command.arg(if path.as_str().is_empty() {
            camino::Utf8Path::new(".")
        } else {
            path
        });
    }
    let status = command.output().ok()?;
    (status.status.success() && status.stdout.is_empty())
        .then(|| current_git_commit(repo).ok())
        .flatten()
}

/// Whether the configured policy turns the Rule binding findings into a
/// refusal. Warning reports the findings and succeeds; error reports them
/// and fails the command. Findings the policy does not govern never refuse
/// here, and Rule severity metadata plays no part in the decision.
fn binding_finding_refusal(
    policy: provenance_store::settings::BindingFindingsSeverity,
    governed: usize,
) -> Option<String> {
    let severity = match policy {
        provenance_store::settings::BindingFindingsSeverity::Warning => {
            provenance_scanner::BindingFindingSeverity::Warning
        }
        provenance_store::settings::BindingFindingsSeverity::Error => {
            provenance_scanner::BindingFindingSeverity::Error
        }
    };
    let refusing = provenance_scanner::binding_findings_fail(severity, governed);
    refusing.then(|| {
        format!(
            "coverage scan found {governed} Rule binding finding(s); the repository \
             configuration selects error"
        )
    })
}

pub(super) fn handle(command: CoverageCommand) -> anyhow::Result<()> {
    match command {
        CoverageCommand::Scan {
            repo,
            path,
            scope,
            baseline,
            validate_rules,
            strict,
            format,
            output,
        } => {
            let (scope, manifest) = RepoContext::new(repo.clone(), scope).declared_scope()?;
            let policy = provenance_store::settings::Settings::load(
                &provenance_store::layout::ProvenanceLayout::new(&repo),
            )?
            .coverage
            .binding_findings;
            let outcome = if let Some(baseline) = baseline.as_deref() {
                coverage_scan_against(
                    &repo,
                    &path,
                    &scope,
                    &manifest,
                    validate_rules,
                    Some(baseline),
                )?
            } else {
                coverage_scan_against(&repo, &path, &scope, &manifest, validate_rules, None)?
            };
            if let Some(output_path) = output {
                let rendered = render_coverage(format, &outcome.scan)?;
                std::fs::write(output_path, rendered)?;
            } else if matches!(format, ReportFormat::Markdown) {
                print!("{}", render_coverage(format, &outcome.scan)?);
            } else {
                output::print_json(&outcome.scan)?;
            }
            if let Some(message) = binding_finding_refusal(policy, outcome.governed_finding_count) {
                anyhow::bail!("{message}");
            }
            if strict && !outcome.scan.warnings.is_empty() {
                anyhow::bail!(
                    "coverage scan found {} warning(s); rerun without --strict to inspect",
                    outcome.scan.warnings.len()
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod git_tests;
