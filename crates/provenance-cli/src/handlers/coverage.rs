use crate::cli::workspace::CoverageCommand;
use crate::output::{self, ReportFormat};
use anyhow::Context;
use camino::Utf8PathBuf;

mod render;
mod verification_state;
use render::render_coverage;
use verification_state::load_validation_state;

pub(super) use provenance_store::evidence_anchors as anchors;

pub(super) fn coverage_scan(
    repo: &camino::Utf8Path,
    path: &Utf8PathBuf,
    scope: &str,
    validate_rules: bool,
) -> anyhow::Result<provenance_core::coverage::CoverageScan> {
    Ok(coverage_scan_against(repo, path, scope, validate_rules, None)?.scan)
}

pub(super) struct CoverageScanOutcome {
    pub(super) scan: provenance_core::coverage::CoverageScan,
    pub(super) governed_finding_count: usize,
}

fn coverage_scan_against(
    repo: &camino::Utf8Path,
    path: &Utf8PathBuf,
    scope: &str,
    validate_rules: bool,
    baseline: Option<&camino::Utf8Path>,
) -> anyhow::Result<CoverageScanOutcome> {
    let scanned = provenance_scanner::scan_path_with_content(path)?;
    coverage_scan_from_scanned_against(repo, path, scope, validate_rules, &scanned, baseline)
}

pub(super) fn coverage_scan_from_scanned(
    repo: &camino::Utf8Path,
    path: &Utf8PathBuf,
    scope: &str,
    scanned: &[provenance_scanner::FileScanWithContent],
) -> anyhow::Result<CoverageScanOutcome> {
    coverage_scan_from_scanned_against(repo, path, scope, true, scanned, None)
}

fn coverage_scan_from_scanned_against(
    repo: &camino::Utf8Path,
    path: &Utf8PathBuf,
    scope: &str,
    validate_rules: bool,
    scanned: &[provenance_scanner::FileScanWithContent],
    baseline: Option<&camino::Utf8Path>,
) -> anyhow::Result<CoverageScanOutcome> {
    let scans = scanned
        .iter()
        .map(|file| file.scan.clone())
        .collect::<Vec<_>>();
    let commit = scan_commit(repo, &scans);
    let validation = load_validation_state(repo, scope, &scans, validate_rules)?;
    // Scanner warnings all come from a line the scan read, so each keeps its
    // location. Derived Rule findings are joined on after, without one.
    let mut warnings = parse_warnings(&scans);
    warnings.extend(validation.warnings);
    let mut governed_finding_count = 0;
    if validate_rules {
        let completeness = if provenance_scanner::scan_covers_repository(repo, path) {
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
    let results = provenance_scanner::coverage_results(&scans);
    let scanned_files = scanned
        .iter()
        .map(|file| provenance_core::coverage::ScannedFile {
            file_path: file.scan.file_path.clone(),
            content: file.content.clone(),
        })
        .collect();
    let mut report = provenance_core::coverage::CoverageScan {
        report: provenance_core::coverage::CoverageReport::new(
            commit,
            scans.len(),
            results.annotations,
            results.bindings,
            warnings,
        ),
        scanned_files,
    };
    report.report.verification_bindings = validation.bindings;
    if let Some(baseline) = baseline {
        let bytes = std::fs::read(baseline)
            .with_context(|| format!("read coverage baseline {baseline}"))?;
        let baseline = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse coverage baseline {baseline}"))?;
        anchors::reconcile(&mut report, &baseline, repo, path, validate_rules);
    }
    Ok(CoverageScanOutcome {
        scan: report,
        governed_finding_count,
    })
}

/// What the parser complained about while reading the files: the legacy
/// Statesman marker, a directive with no `key: value`, a confidence
/// outside the range, an unknown field.
///
/// These name a file and a line but no rule. A malformed directive may never
/// get as far as saying which rule it meant, and inventing one would send a
/// reader after a rule that does not exist. The empty `rule_id` is what an
/// absent subject looks like, and the render leaves it out.
///
/// They are reported whether or not `--validate-rules` was passed: nothing
/// here is checked against the graph, so the scan owes them either way. Before
/// this, every one of them was dropped on the floor.
fn parse_warnings(
    scans: &[provenance_scanner::FileScan],
) -> Vec<provenance_core::coverage::ValidationWarning> {
    scans
        .iter()
        .flat_map(|scan| {
            scan.warnings
                .iter()
                .map(|warning| provenance_core::coverage::ValidationWarning {
                    rule_id: String::new(),
                    file_path: Some(scan.file_path.clone()),
                    line: Some(warning.line),
                    message: warning.message.clone(),
                    binding_finding: false,
                })
        })
        .collect()
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
        format!("coverage scan found {governed} Rule binding finding(s); the repository configuration selects error")
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
            let policy = provenance_store::settings::Settings::load(
                &provenance_store::layout::ProvenanceLayout::new(&repo),
            )?
            .coverage
            .binding_findings;
            let outcome = if let Some(baseline) = baseline.as_deref() {
                coverage_scan_against(&repo, &path, &scope, validate_rules, Some(baseline))?
            } else {
                coverage_scan_against(&repo, &path, &scope, validate_rules, None)?
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

#[cfg(test)]
mod parse_warning_tests;
