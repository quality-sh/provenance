//! The deterministic Markdown renderer for report envelopes.
//!
//! Normalize first: equivalent input with reordered records, relationships,
//! findings or evidence sites renders byte-identical UTF-8 Markdown. Fixed
//! sentence templates and a fixed budget bound the output. Untrusted text is
//! data: graph statements are quoted, never executed as formatting.

mod findings;
mod order;
mod sections;

use crate::report::envelope::ReportEnvelope;
use provenance_macros::rule;
use std::fmt::Write as _;

/// Maximum finding sections rendered before an explicit omission line.
pub const MAX_RENDERED_FINDINGS: usize = 10;
/// Maximum graph-change table rows before an explicit omission line.
pub const MAX_GRAPH_CHANGE_ROWS: usize = 40;

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
        envelope.repository, envelope.scope, envelope.base_commit, envelope.head_commit
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
