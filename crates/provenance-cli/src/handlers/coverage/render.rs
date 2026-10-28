//! One renderer for the scan report: markdown for people reading the run,
//! JSON for the machine that archives it.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::output::ReportFormat;
use provenance_core::coverage::{CoverageReport, CoverageSite, SiteRole};

/// Said of a verification site that lives in a different file from the
/// primary implementation binding it checks.
const OUTSIDE_IMPLEMENTATION_MODULE: &str = " (outside implementation module)";

fn site_details(site: CoverageSite<'_>) -> String {
    let name = site
        .symbol()
        .map(|name| format!(" ({name})"))
        .unwrap_or_default();
    match site {
        CoverageSite::Annotation(annotation) => format!("{name} ({})", annotation.coverage),
        CoverageSite::Binding(_) => name,
    }
}

/// Where each rule is implemented: the file holding its native or portable
/// binding with no verification method.
///
/// A rule with no `#[rule]` site in the scanned tree is absent from the map,
/// and its verification sites are then left unannotated. Nothing is known
/// about where it belongs, so nothing is claimed.
fn implementation_modules(report: &CoverageReport) -> BTreeMap<&str, &camino::Utf8Path> {
    let mut selected = BTreeMap::new();
    for site in report
        .sites()
        .filter(|site| matches!(site.role(), SiteRole::Implementation) && site.is_current())
    {
        let core = site.core();
        match site {
            CoverageSite::Annotation(_) => {
                selected.insert(core.rule_id.as_str(), (core.file_path.as_path(), true));
            }
            CoverageSite::Binding(_) => {
                let previous_is_annotation = selected
                    .get(core.rule_id.as_str())
                    .is_some_and(|(_, annotation)| *annotation);
                if !previous_is_annotation {
                    selected.insert(core.rule_id.as_str(), (core.file_path.as_path(), false));
                }
            }
        }
    }
    selected
        .into_iter()
        .map(|(rule_id, (path, _))| (rule_id, path))
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
    format: ReportFormat,
    report: &provenance_core::coverage::CoverageScan,
) -> anyhow::Result<String> {
    if matches!(format, ReportFormat::Markdown) {
        let mut out = String::from("# Coverage Scan\n\n");
        writeln!(out, "- Files scanned: {}", report.files_scanned)?;
        writeln!(out, "- Total annotations: {}", report.total_annotations)?;
        writeln!(out, "- Warnings: {}\n", report.warnings.len())?;
        let implementation_modules = implementation_modules(report);
        for report_site in report.sites() {
            let site = report_site.core();
            let role = report_site.role();
            let relation = match role {
                SiteRole::Implementation => "is implemented".to_string(),
                SiteRole::Verification(method) => format!("verified by {method}"),
            };
            writeln!(
                out,
                "- `{}` {} at `{}`:{}{}{}{}",
                site.rule_id,
                relation,
                site.file_path,
                site.line,
                site_details(report_site),
                anchor_state(
                    site.anchor_state,
                    site.original_line,
                    site.original_file_path.as_deref()
                ),
                if is_outside_implementation_module(
                    &site.rule_id,
                    &site.file_path,
                    matches!(role, SiteRole::Verification(_)),
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

#[cfg(test)]
mod tests;
