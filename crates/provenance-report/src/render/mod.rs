//! The deterministic Markdown renderer for report envelopes.
//!
//! Normalize first: equivalent input with reordered records, relationships,
//! findings or evidence sites renders byte-identical UTF-8 Markdown. Fixed
//! sentence templates and a fixed budget bound the output. Untrusted text is
//! data: graph statements are quoted, never executed as formatting.

mod findings;
mod order;
mod sections;

use crate::envelope::ReportEnvelope;
use crate::escape::escape_inline;
use provenance_macros::rule;
use serde::Serialize;
use std::cmp::Ordering;
use std::fmt::Write as _;

/// Maximum finding sections rendered before an explicit omission line.
pub const MAX_RENDERED_FINDINGS: usize = 10;
/// Maximum graph-change table rows before an explicit omission line.
pub const MAX_GRAPH_CHANGE_ROWS: usize = 40;
/// Maximum items in one site, removed-site or run list inside a finding.
pub const MAX_LIST_ITEMS: usize = 5;

/// Refuse duplicate identities whose payloads differ.
///
/// Byte-equal duplicates may repeat: they render identically under any order.
/// Different payloads under one sort identity would render in input order and
/// break the byte-identity guarantee, so the envelope is refused instead.
pub fn validate_duplicates(envelope: &ReportEnvelope) -> Result<(), String> {
    let mut changes = envelope.graph_changes.clone();
    changes.sort_by(order::compare_graph_changes);
    check_adjacent(&changes, order::compare_graph_changes, "graph change")?;
    let mut findings = envelope.findings.clone();
    findings.sort_by(order::compare_findings);
    check_adjacent(&findings, order::compare_findings, "finding")?;
    for finding in &findings {
        let mut sites = finding.sites.clone();
        sites.extend(finding.removed_sites.iter().cloned());
        sites.sort_by(order::compare_sites);
        check_adjacent(&sites, order::compare_sites, "site")?;
    }
    Ok(())
}

fn check_adjacent<T: Serialize>(
    items: &[T],
    compare: impl Fn(&T, &T) -> Ordering,
    what: &str,
) -> Result<(), String> {
    for pair in items.windows(2) {
        let same_payload = |item: &T, other: &T| {
            serde_json::to_string(item).ok() == serde_json::to_string(other).ok()
        };
        if compare(&pair[0], &pair[1]) == Ordering::Equal && !same_payload(&pair[0], &pair[1]) {
            return Err(format!(
                "duplicate {what} identity with a different payload; identical \
                 payloads may repeat, different payloads need distinct identities"
            ));
        }
    }
    Ok(())
}

/// Sort every collection in the envelope into the canonical order. Input and
/// graph shard order never reach the output.
pub fn normalize(envelope: &ReportEnvelope) -> ReportEnvelope {
    let mut sorted = envelope.clone();
    sorted.graph_changes.sort_by(order::compare_graph_changes);
    for change in &mut sorted.graph_changes {
        change.relations_added.sort_by(order::compare_relations);
        change.relations_removed.sort_by(order::compare_relations);
    }
    sorted.findings.sort_by(order::compare_findings);
    for finding in &mut sorted.findings {
        finding.sites.sort_by(order::compare_sites);
        finding.removed_sites.sort_by(order::compare_sites);
    }
    sorted
        .verification_runs
        .sort_by(order::compare_verification_runs);
    sorted
}

/// Render the normalized envelope as bounded Markdown.
#[rule("rule_report_render_is_deterministic")]
pub fn render_markdown(envelope: &ReportEnvelope) -> String {
    let mut out = String::new();
    out.push_str("# Provenance report\n\n");
    writeln!(
        out,
        "Repository: {} · Scope: {} · Comparison: {} → {}\n",
        envelope.repository,
        escape_inline(&envelope.scope),
        envelope.base_commit,
        envelope.head_commit
    )
    .expect("writing to a String should not fail");
    sections::scan_section(envelope, &mut out);
    sections::policy_section(envelope, &mut out);
    sections::graph_changes_section(envelope, &mut out);
    findings::findings_section(envelope, &mut out);
    out.push_str(
        "---\n\nRendered by `provenance report render` from the report \
         envelope. The text is generated deterministically; no language \
         model writes this report.\n",
    );
    out
}
