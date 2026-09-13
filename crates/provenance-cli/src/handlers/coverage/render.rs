//! One renderer for the scan report: markdown for people reading the run,
//! JSON for the machine that archives it.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::output::ReportFormat;
use provenance_core::coverage::{AnnotationResult, BindingResult, CoverageReport, SiteCore};

/// Said of a verification site that lives in a different file from the
/// primary implementation binding it checks.
const OUTSIDE_IMPLEMENTATION_MODULE: &str = " (outside implementation module)";

#[derive(Clone, Copy)]
enum ReportSite<'a> {
    Annotation(&'a AnnotationResult),
    Binding(&'a BindingResult),
}

impl<'a> ReportSite<'a> {
    const fn core(self) -> &'a SiteCore {
        match self {
            Self::Annotation(site) => &site.site,
            Self::Binding(site) => &site.site,
        }
    }

    fn details(self) -> String {
        let name = match self {
            Self::Annotation(site) => site.function_name.as_deref(),
            Self::Binding(site) => site.item_name.as_deref(),
        }
        .map(|name| format!(" ({name})"))
        .unwrap_or_default();
        match self {
            Self::Annotation(site) => format!("{name} ({})", site.coverage),
            Self::Binding(_) => name,
        }
    }
}

fn report_sites(report: &CoverageReport) -> impl Iterator<Item = ReportSite<'_>> {
    report
        .annotations
        .iter()
        .map(ReportSite::Annotation)
        .chain(report.bindings.iter().map(ReportSite::Binding))
}

fn implementation_sites(report: &CoverageReport) -> impl Iterator<Item = ReportSite<'_>> {
    report
        .bindings
        .iter()
        .map(ReportSite::Binding)
        .chain(report.annotations.iter().map(ReportSite::Annotation))
}

/// Where each rule is implemented: the file holding its native or portable
/// binding with no verification method.
///
/// A rule with no `#[rule]` site in the scanned tree is absent from the map,
/// and its verification sites are then left unannotated. Nothing is known
/// about where it belongs, so nothing is claimed.
fn implementation_modules(report: &CoverageReport) -> BTreeMap<&str, &camino::Utf8Path> {
    implementation_sites(report)
        .map(ReportSite::core)
        .filter(|site| {
            site.verification.is_none()
                && site.anchor_state != provenance_core::coverage::AnchorState::Gone
        })
        .map(|site| (site.rule_id.as_str(), site.file_path.as_path()))
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
        for report_site in report_sites(report) {
            let site = report_site.core();
            let relation = site.verification.as_ref().map_or_else(
                || "is implemented".to_string(),
                |method| format!("verified by {method}"),
            );
            writeln!(
                out,
                "- `{}` {} at `{}`:{}{}{}{}",
                site.rule_id,
                relation,
                site.file_path,
                site.line,
                report_site.details(),
                anchor_state(
                    site.anchor_state,
                    site.original_line,
                    site.original_file_path.as_deref()
                ),
                if is_outside_implementation_module(
                    &site.rule_id,
                    &site.file_path,
                    site.verification.is_some(),
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
