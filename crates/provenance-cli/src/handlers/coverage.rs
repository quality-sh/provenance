use crate::cli::workspace::CoverageCommand;
use crate::output::{self, OutputFormat};
use anyhow::Context;
use camino::Utf8PathBuf;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

mod retired;
mod verification_state;
use verification_state::{load_validation_state, unverified_rule_warnings};

pub(super) use provenance_store::evidence_anchors as anchors;

pub(super) fn coverage_scan(
    repo: &camino::Utf8Path,
    path: &Utf8PathBuf,
    scope: &str,
    validate_rules: bool,
) -> anyhow::Result<provenance_core::coverage::CoverageScan> {
    coverage_scan_against(repo, path, scope, validate_rules, None)
}

fn coverage_scan_against(
    repo: &camino::Utf8Path,
    path: &Utf8PathBuf,
    scope: &str,
    validate_rules: bool,
    baseline: Option<&camino::Utf8Path>,
) -> anyhow::Result<provenance_core::coverage::CoverageScan> {
    let scanned = provenance_scanner::scan_path_with_content(path)?;
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
    if validate_rules {
        // A marker citing a retired Rule is a fact about a line the scan
        // read, so it stands even when the scan covers part of the tree.
        // Derived absence needs the whole repository to be honest.
        warnings.extend(retired::stale_rule_warnings(&validation.rules, &scans));
        if scan_covers_repository(repo, path) {
            warnings.extend(unimplemented_rule_warnings(
                &validation.rules,
                &scans,
                &validation.implementations,
            ));
            warnings.extend(unverified_rule_warnings(
                &validation.rules,
                &scans,
                &validation.bindings,
            ));
        }
    }
    let annotations = scans
        .iter()
        .flat_map(|scan| &scan.annotations)
        .map(|location| provenance_core::coverage::AnnotationResult {
            rule_id: location.annotation.rule.clone(),
            file_path: location.file_path.clone(),
            line: location.line,
            function_name: location.function_name.clone(),
            coverage: location.annotation.coverage.to_string(),
            confidence: location.annotation.confidence,
            verification: location
                .annotation
                .verification
                .map(|method| method.to_string()),
            anchor: Some(location.anchor.clone()),
            anchor_state: provenance_core::coverage::AnchorState::New,
            original_line: None,
            original_file_path: None,
        })
        .collect::<Vec<_>>();
    let bindings = scans
        .iter()
        .flat_map(|scan| &scan.bindings)
        .map(|binding| provenance_core::coverage::BindingResult {
            rule_id: binding.rule_id.clone(),
            file_path: binding.file_path.clone(),
            line: binding.line,
            item_name: binding.item_name.clone(),
            verification: binding.verification.map(|method| method.to_string()),
            anchor: Some(binding.anchor.clone()),
            anchor_state: provenance_core::coverage::AnchorState::New,
            original_line: None,
            original_file_path: None,
        })
        .collect::<Vec<_>>();
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
            annotations,
            bindings,
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
    Ok(report)
}

/// Partial scans validate encountered bindings but cannot claim that a Rule
/// has no implementation or verification elsewhere in the repository.
fn scan_covers_repository(repo: &camino::Utf8Path, path: &camino::Utf8Path) -> bool {
    same_file::is_same_file(repo, path).unwrap_or(false)
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
                })
        })
        .collect()
}

/// An active Rule with no scanned primary implementation site is
/// unimplemented. A verification site and a persisted source citation do not
/// count: evidence, source material, and implementation are distinct.
fn unimplemented_rule_warnings(
    rules: &[provenance_core::Rule],
    scans: &[provenance_scanner::FileScan],
    typed_bindings: &[provenance_core::ImplementationBinding],
) -> Vec<provenance_core::coverage::ValidationWarning> {
    let mut implementations = provenance_scanner::source_sites(scans)
        .filter(|site| site.role() == provenance_scanner::SourceSiteRole::Implementation)
        .map(|site| site.rule_id().to_string())
        .collect::<BTreeSet<_>>();
    implementations.extend(
        typed_bindings
            .iter()
            .map(|binding| binding.rule_id.as_str().to_string()),
    );
    rules
        .iter()
        .filter(|rule| rule.status == provenance_core::RuleStatus::Active && !rule.retired)
        .filter(|rule| !implementations.contains(rule.id.as_str()))
        .map(|rule| provenance_core::coverage::ValidationWarning {
            rule_id: rule.id.as_str().to_string(),
            file_path: None,
            line: None,
            message: format!("active rule `{}` has no implementation", rule.id.as_str()),
        })
        .collect()
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

/// Said of a verification site that lives in a different file from the
/// primary implementation binding it checks.
const OUTSIDE_IMPLEMENTATION_MODULE: &str = " (outside implementation module)";

/// Where each rule is implemented: the file holding its native or portable
/// binding with no verification method.
///
/// A rule with no `#[rule]` site in the scanned tree is absent from the map,
/// and its verification sites are then left unannotated. Nothing is known
/// about where it belongs, so nothing is claimed.
fn implementation_modules(
    report: &provenance_core::coverage::CoverageReport,
) -> BTreeMap<&str, &camino::Utf8Path> {
    report
        .bindings
        .iter()
        .filter(|binding| {
            binding.verification.is_none()
                && binding.anchor_state != provenance_core::coverage::AnchorState::Gone
        })
        .map(|binding| (binding.rule_id.as_str(), binding.file_path.as_path()))
        .chain(
            report
                .annotations
                .iter()
                .filter(|site| {
                    site.verification.is_none()
                        && site.anchor_state != provenance_core::coverage::AnchorState::Gone
                })
                .map(|site| (site.rule_id.as_str(), site.file_path.as_path())),
        )
        .collect()
}

/// Whether this site checks a rule implemented somewhere else.
///
/// This is what a change author wants out of the report: the sites that lean
/// on the implementation from another module, which is where a change to the
/// implementation breaks somebody else's tests.
fn is_outside_implementation_module(
    rule_id: &str,
    file_path: &camino::Utf8Path,
    is_verification: bool,
    implementation_modules: &BTreeMap<&str, &camino::Utf8Path>,
) -> bool {
    is_verification
        && implementation_modules
            .get(rule_id)
            .is_some_and(|implementation| *implementation != file_path)
}

/// The rule a warning is about, ready to sit after the word `Warning`.
///
/// Empty for a warning about no rule in particular, and then the line reads
/// `- Warning in `path`:line: message`. An empty pair of backticks would only
/// look like a rule whose name went missing.
fn warning_subject(rule_id: &str) -> String {
    if rule_id.is_empty() {
        String::new()
    } else {
        format!(" `{rule_id}`")
    }
}

fn anchor_state(
    state: provenance_core::coverage::AnchorState,
    original_line: Option<usize>,
    original_file_path: Option<&camino::Utf8Path>,
) -> String {
    match (state, original_line) {
        (provenance_core::coverage::AnchorState::Moved, Some(line)) => original_file_path
            .map_or_else(
                || format!(" (moved from line {line})"),
                |file| format!(" (moved from {file}:{line})"),
            ),
        (provenance_core::coverage::AnchorState::Moved, None) => " (moved)".to_string(),
        (provenance_core::coverage::AnchorState::Gone, _) => " (gone)".to_string(),
        (provenance_core::coverage::AnchorState::New, _) => " (new)".to_string(),
        (provenance_core::coverage::AnchorState::Unchanged, _) => String::new(),
    }
}

pub(super) fn render_coverage(
    format: OutputFormat,
    report: &provenance_core::coverage::CoverageScan,
) -> anyhow::Result<String> {
    if matches!(format, OutputFormat::Markdown) {
        let mut out = String::from("# Coverage Scan\n\n");
        writeln!(out, "- Files scanned: {}", report.files_scanned)?;
        writeln!(out, "- Total annotations: {}", report.total_annotations)?;
        writeln!(out, "- Warnings: {}\n", report.warnings.len())?;
        let implementation_modules = implementation_modules(report);
        for annotation in &report.annotations {
            let relation = annotation.verification.as_ref().map_or_else(
                || "is implemented".to_string(),
                |method| format!("verified by {method}"),
            );
            writeln!(
                out,
                "- `{}` {} at `{}`:{}{} ({}){}{}",
                annotation.rule_id,
                relation,
                annotation.file_path,
                annotation.line,
                annotation
                    .function_name
                    .as_deref()
                    .map(|name| format!(" ({name})"))
                    .unwrap_or_default(),
                annotation.coverage,
                anchor_state(
                    annotation.anchor_state,
                    annotation.original_line,
                    annotation.original_file_path.as_deref()
                ),
                if is_outside_implementation_module(
                    &annotation.rule_id,
                    &annotation.file_path,
                    annotation.verification.is_some(),
                    &implementation_modules,
                ) {
                    OUTSIDE_IMPLEMENTATION_MODULE
                } else {
                    ""
                }
            )?;
        }
        for binding in &report.bindings {
            let relation = binding.verification.as_ref().map_or_else(
                || "is implemented".to_string(),
                |method| format!("verified by {method}"),
            );
            writeln!(
                out,
                "- `{}` {} at `{}`:{}{}{}{}",
                binding.rule_id,
                relation,
                binding.file_path,
                binding.line,
                binding
                    .item_name
                    .as_deref()
                    .map(|name| format!(" ({name})"))
                    .unwrap_or_default(),
                anchor_state(
                    binding.anchor_state,
                    binding.original_line,
                    binding.original_file_path.as_deref()
                ),
                if is_outside_implementation_module(
                    &binding.rule_id,
                    &binding.file_path,
                    binding.verification.is_some(),
                    &implementation_modules,
                ) {
                    OUTSIDE_IMPLEMENTATION_MODULE
                } else {
                    ""
                }
            )?;
        }
        for warning in &report.warnings {
            let subject = warning_subject(&warning.rule_id);
            match (&warning.file_path, warning.line) {
                (Some(file_path), Some(line)) => writeln!(
                    out,
                    "- Warning{subject} in `{file_path}`:{line}: {}",
                    warning.message
                )?,
                _ => writeln!(out, "- Warning{subject}: {}", warning.message)?,
            }
        }
        Ok(out)
    } else {
        Ok(serde_json::to_string_pretty(report)?)
    }
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
            let report = if let Some(baseline) = baseline.as_deref() {
                coverage_scan_against(&repo, &path, &scope, validate_rules, Some(baseline))?
            } else {
                coverage_scan(&repo, &path, &scope, validate_rules)?
            };
            if let Some(output_path) = output {
                let rendered = render_coverage(format, &report)?;
                std::fs::write(output_path, rendered)?;
            } else if matches!(format, OutputFormat::Markdown | OutputFormat::Toon) {
                print!("{}", render_coverage(format, &report)?);
            } else {
                output::print(format, &report)?;
            }
            if strict && !report.warnings.is_empty() {
                anyhow::bail!(
                    "coverage scan found {} warning(s); rerun without --strict to inspect",
                    report.warnings.len()
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod git_tests;

#[cfg(test)]
mod implementation_tests;

#[cfg(test)]
mod parse_warning_tests;

#[cfg(test)]
mod render_tests;

#[cfg(test)]
mod unverified_tests;
