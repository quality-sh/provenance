//! Scan, policy and graph-change sections of the report.

use crate::envelope::{
    BaselineCompatibility, Completeness, GraphChange, PolicyMode, PolicyResult, RelationChange,
    ReportEnvelope, ScanStage,
};
use crate::escape::{escape_cell, escape_inline};
use std::fmt::Write as _;

pub(super) fn scan_section(envelope: &ReportEnvelope, out: &mut String) {
    out.push_str("## Scan\n\n");
    match envelope.scan.completeness {
        Completeness::Complete => {
            writeln!(
                out,
                "- Scan complete: {} files.",
                envelope.scan.files_scanned
            )
            .expect("writing to a String should not fail");
        }
        Completeness::Incomplete => {
            let reason = envelope
                .scan
                .incompleteness_reason
                .as_deref()
                .unwrap_or("reason not supplied");
            writeln!(
                out,
                "- Scan incomplete: {}. Absence findings are not established.",
                escape_inline(reason)
            )
            .expect("writing to a String should not fail");
        }
    }
    match envelope.scan.baseline {
        BaselineCompatibility::Compatible => out.push_str("- Baseline compatible.\n"),
        BaselineCompatibility::Missing => {
            out.push_str("- Baseline missing. Findings are not labelled new or resolved.\n");
        }
        BaselineCompatibility::Incompatible => {
            let reason = envelope
                .scan
                .baseline_reason
                .as_deref()
                .unwrap_or("reason not supplied");
            writeln!(
                out,
                "- Baseline incompatible: {}. Findings are not labelled new or \
                 resolved.",
                escape_inline(reason)
            )
            .expect("writing to a String should not fail");
        }
    }
    if let Some(failure) = &envelope.scan.failure {
        writeln!(
            out,
            "- Operational failure at {}: {}. The result is unavailable, not \
             clean.",
            stage_name(failure.stage),
            escape_inline(&failure.message)
        )
        .expect("writing to a String should not fail");
    }
    out.push('\n');
}

pub(super) fn policy_section(envelope: &ReportEnvelope, out: &mut String) {
    let Some(policy) = &envelope.policy else {
        return;
    };
    out.push_str("## Policy\n\n");
    let mode = match policy.mode {
        PolicyMode::Warning => "warning",
        PolicyMode::Error => "error",
    };
    let result = match policy.result {
        PolicyResult::Success => "success; the check passes",
        PolicyResult::Failure => "failure; the check fails",
        PolicyResult::Unavailable => "unavailable; the check did not run",
    };
    writeln!(out, "- Mode: {mode}. Result: {result}.\n")
        .expect("writing to a String should not fail");
}

pub(super) fn graph_changes_section(envelope: &ReportEnvelope, out: &mut String) {
    if envelope.graph_changes.is_empty() {
        return;
    }
    out.push_str("## Graph changes\n\n");
    out.push_str("| Change | Record | Statement | Details |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    let limit = crate::render::MAX_GRAPH_CHANGE_ROWS;
    let omitted = envelope.graph_changes.len().saturating_sub(limit);
    for change in envelope.graph_changes.iter().take(limit) {
        writeln!(
            out,
            "| {} | {} `{}` | {} | {} |",
            change.change.as_str(),
            change.kind.as_str(),
            escape_cell(&change.id),
            statement_cell(change),
            details_cell(change)
        )
        .expect("writing to a String should not fail");
    }
    if omitted > 0 {
        writeln!(out, "\n{omitted} more graph changes omitted.")
            .expect("writing to a String should not fail");
    }
    out.push('\n');
}

fn statement_cell(change: &GraphChange) -> String {
    let text = change.statement.as_deref().unwrap_or("");
    if let Some(before) = change.statement_before.as_deref() {
        return format!("{} → {}", escape_cell(before), escape_cell(text));
    }
    escape_cell(text)
}

fn details_cell(change: &GraphChange) -> String {
    let mut parts = Vec::new();
    if let Some(before) = change.lifecycle_before.as_deref() {
        let after = change.lifecycle_after.as_deref().unwrap_or(before);
        parts.push(format!("status: {before} → {after}"));
    }
    for relation in &change.relations_added {
        parts.push(relation_text("adds", relation));
    }
    for relation in &change.relations_removed {
        parts.push(relation_text("removes", relation));
    }
    escape_cell(&parts.join("; "))
}

fn relation_text(verb: &str, relation: &RelationChange) -> String {
    format!(
        "{verb} {} → {} `{}`",
        relation.relation,
        relation.target_kind.as_str(),
        relation.target_id
    )
}

const fn stage_name(stage: ScanStage) -> &'static str {
    match stage {
        ScanStage::GraphDecode => "graph decode",
        ScanStage::GraphCheck => "graph check",
        ScanStage::SourceScan => "source scan",
        ScanStage::BaselineLoad => "baseline load",
        ScanStage::EvidenceCollection => "evidence collection",
        ScanStage::VerificationCollection => "verification collection",
    }
}
