//! Findings sections: one fixed-template block per finding.

use super::order::finding_class;
use crate::catalog::DiagnosticCode;
use crate::envelope::{
    BindingPresence, CommitRole, Comparison, Finding, Relevance, ReportEnvelope, RunStatus,
    Severity, Site, SiteRole, SubjectKind, VerificationRun,
};
use crate::escape::escape_inline;
use std::fmt::Write as _;

pub(super) fn findings_section(envelope: &ReportEnvelope, out: &mut String) {
    if envelope.findings.is_empty() {
        if envelope.scan.failure.is_some() {
            out.push_str(
                "## Findings\n\nThe scan failed; findings are unavailable, \
                 not clean.\n\n",
            );
        }
        return;
    }
    out.push_str("## Findings\n\n");
    writeln!(out, "{}\n", findings_summary(envelope)).expect("writing to a String should not fail");
    let limit = super::MAX_RENDERED_FINDINGS;
    let omitted = envelope.findings.len().saturating_sub(limit);
    for finding in envelope.findings.iter().take(limit) {
        finding_section(envelope, finding, out);
    }
    if omitted > 0 {
        writeln!(out, "{omitted} more findings omitted.")
            .expect("writing to a String should not fail");
    }
}

const CLASS_SINGULAR: [&str; 6] = [
    "new error",
    "new warning",
    "changed-intent review",
    "resolved finding",
    "pre-existing finding",
    "uncertain finding",
];
const CLASS_PLURAL: [&str; 6] = [
    "new errors",
    "new warnings",
    "changed-intent reviews",
    "resolved findings",
    "pre-existing findings",
    "uncertain findings",
];

fn findings_summary(envelope: &ReportEnvelope) -> String {
    let mut counts = [0usize; 6];
    for finding in &envelope.findings {
        counts[finding_class(finding) as usize] += 1;
    }
    let parts: Vec<String> = counts
        .iter()
        .enumerate()
        .filter(|(_, count)| **count > 0)
        .map(|(class, count)| match count {
            1 => format!("1 {}", CLASS_SINGULAR[class]),
            n => format!("{n} {}", CLASS_PLURAL[class]),
        })
        .collect();
    let total: usize = counts.iter().sum();
    let noun = if total == 1 { "finding" } else { "findings" };
    if parts.is_empty() {
        return format!("{total} {noun}.");
    }
    format!("{total} {noun}: {}; shown.", parts.join(", "))
}

fn finding_section(envelope: &ReportEnvelope, finding: &Finding, out: &mut String) {
    let subject_word = match finding.subject.kind {
        SubjectKind::Rule => "Rule",
        SubjectKind::Requirement => "Requirement",
    };
    let code = DiagnosticCode::parse(&finding.code);
    let headline = code.map_or(finding.code.as_str(), |known| known.headline());
    writeln!(
        out,
        "### {}: {}\n",
        severity_name(finding.severity),
        headline
    )
    .expect("writing to a String should not fail");
    writeln!(
        out,
        "- {subject_word}: `{}`.",
        escape_inline(&finding.subject.id)
    )
    .expect("writing to a String should not fail");
    if let Some(requirement) = &finding.affected_requirement_id {
        writeln!(out, "- Refines: `{}`.", escape_inline(requirement))
            .expect("writing to a String should not fail");
    }
    if let Some(rule) = &finding.affected_rule_id {
        writeln!(out, "- Affected Rule: `{}`.", escape_inline(rule))
            .expect("writing to a String should not fail");
    }
    if let Some(statement) = &finding.statement {
        writeln!(
            out,
            "- The {subject_word} says: “{}”",
            escape_inline(statement)
        )
        .expect("writing to a String should not fail");
    }
    writeln!(
        out,
        "- Comparison: {} ({} → {}).",
        comparison_text(finding.comparison),
        envelope.base_commit,
        envelope.head_commit
    )
    .expect("writing to a String should not fail");
    if finding.relevance == Relevance::ReviewRequested {
        out.push_str(
            "- Evidence relevance: a Requirement review asks for new \
             evidence.\n",
        );
    }
    out.push_str(&presence_line(envelope, finding));
    out.push_str(&removed_sites_line(envelope, finding));
    out.push_str(&runs_lines(envelope, finding));
    let action = code.map_or(finding.code.as_str(), |known| known.next_action());
    writeln!(out, "- Next action: {action}\n").expect("writing to a String should not fail");
}

fn presence_line(envelope: &ReportEnvelope, finding: &Finding) -> String {
    match finding.binding_presence {
        BindingPresence::Absent => "- Current bindings: none.\n".to_string(),
        BindingPresence::Unknown => "- Current bindings: unknown.\n".to_string(),
        BindingPresence::Present if finding.sites.is_empty() => {
            "- Current bindings: present; no site locations supplied.\n".to_string()
        }
        BindingPresence::Present => {
            let implementation: Vec<String> = finding
                .sites
                .iter()
                .filter(|site| site.role == Some(SiteRole::Implementation))
                .map(|site| site_text(envelope, site))
                .collect();
            let verification: Vec<String> = finding
                .sites
                .iter()
                .filter(|site| site.role == Some(SiteRole::Verification))
                .map(|site| site_text(envelope, site))
                .collect();
            let unroled: Vec<String> = finding
                .sites
                .iter()
                .filter(|site| site.role.is_none())
                .map(|site| site_text(envelope, site))
                .collect();
            let implementation = bounded_list(implementation, "sites");
            let verification = bounded_list(verification, "sites");
            let unroled = bounded_list(unroled, "sites");
            let mut line = String::new();
            if !implementation.is_empty() {
                writeln!(line, "- Current implementation: {implementation}.")
                    .expect("writing to a String should not fail");
            }
            if !verification.is_empty() {
                writeln!(line, "- Current verification: {verification}.")
                    .expect("writing to a String should not fail");
            }
            if !unroled.is_empty() {
                writeln!(line, "- Current bindings: {unroled}.")
                    .expect("writing to a String should not fail");
            }
            line
        }
    }
}

fn removed_sites_line(envelope: &ReportEnvelope, finding: &Finding) -> String {
    if finding.removed_sites.is_empty() {
        return String::new();
    }
    let items: Vec<String> = finding
        .removed_sites
        .iter()
        .map(|site| site_text(envelope, site))
        .collect();
    let joined = bounded_list(items, "removed sites");
    format!("- Removed evidence: {joined}.\n")
}

/// Join one bounded list. The per-finding budget keeps a large envelope from
/// producing unbounded output, and the cut is stated in the rendered line.
fn bounded_list(items: Vec<String>, what: &str) -> String {
    let limit = super::MAX_LIST_ITEMS;
    let omitted = items.len().saturating_sub(limit);
    let mut joined = items.into_iter().take(limit).collect::<Vec<_>>().join(", ");
    if omitted > 0 {
        write!(joined, " ({omitted} more {what} omitted)")
            .expect("writing to a String should not fail");
    }
    joined
}

/// One site as a link built from an immutable commit plus a validated
/// repository-relative path. A path that fails validation renders as plain
/// text with no URL; an author-supplied URL is never a repository link.
fn site_text(envelope: &ReportEnvelope, site: &Site) -> String {
    let label = format!("{}:{}", site.path, site.line);
    let commit = match site.commit {
        CommitRole::Base => &envelope.base_commit,
        CommitRole::Head => &envelope.head_commit,
    };
    let location = repository_link(envelope, commit, &site.path, site.line)
        .map_or_else(|| format!("`{label}`"), |url| format!("[`{label}`]({url})"));
    match (&site.role, site.method.as_deref()) {
        (Some(SiteRole::Verification), Some(method)) => {
            format!("{location} (method `{}`)", escape_inline(method))
        }
        _ => location,
    }
}

fn repository_link(
    envelope: &ReportEnvelope,
    commit: &str,
    path: &str,
    line: u32,
) -> Option<String> {
    if !crate::envelope::is_repo_relative_path(path) {
        return None;
    }
    Some(format!(
        "https://github.com/{}/blob/{commit}/{path}#L{line}",
        envelope.repository
    ))
}

fn runs_lines(envelope: &ReportEnvelope, finding: &Finding) -> String {
    let matching: Vec<&VerificationRun> = envelope
        .verification_runs
        .iter()
        .filter(|run| run.rule_id == finding.subject.id)
        .collect();
    if matching.is_empty() {
        return "- Verification run: not supplied.\n".to_string();
    }
    let limit = super::MAX_LIST_ITEMS;
    let omitted = matching.len().saturating_sub(limit);
    let mut lines = String::new();
    for run in matching.into_iter().take(limit) {
        // Run fields are untrusted envelope text: escape before interpolation.
        let method = escape_inline(run.method.as_deref().unwrap_or("verification"));
        let at = escape_inline(run.commit.as_deref().unwrap_or("an unknown commit"));
        let line = match (run.status, run.commit.as_deref()) {
            (RunStatus::Passed, Some(commit)) if commit == envelope.head_commit => {
                format!("- Verification run ({method}): passed at {at}.\n")
            }
            (RunStatus::Passed, _) => format!(
                "- Verification run ({method}): passed at {at}. This run is \
                 not for the head commit.\n"
            ),
            (RunStatus::Failed, _) => {
                format!("- Verification run ({method}): failed at {at}.\n")
            }
            (RunStatus::Running, _) => {
                format!("- Verification run ({method}): running.\n")
            }
        };
        lines.push_str(&line);
    }
    if omitted > 0 {
        writeln!(lines, "- {omitted} more verification runs omitted.")
            .expect("writing to a String should not fail");
    }
    lines
}

const fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}

const fn comparison_text(comparison: Comparison) -> &'static str {
    match comparison {
        Comparison::New => "new in this comparison",
        Comparison::PreExisting => "pre-existing; present at the base",
        Comparison::Resolved => "resolved in this comparison; absent at the head",
        Comparison::Uncertain => "uncertain; the comparison cannot classify this finding",
    }
}
