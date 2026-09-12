//! Stable ordering for normalized report envelopes.
//!
//! Display class order: new errors, new warnings, changed-intent reviews,
//! resolved, pre-existing, then uncertain. Within one class, identity decides:
//! diagnostic code, then subject id, then the first site.

use crate::catalog::DiagnosticCode;
use crate::envelope::{
    Comparison, Finding, GraphChange, RelationChange, Severity, Site, VerificationRun,
};

pub(crate) fn compare_graph_changes(a: &GraphChange, b: &GraphChange) -> std::cmp::Ordering {
    (a.kind, a.change, a.id.as_str()).cmp(&(b.kind, b.change, b.id.as_str()))
}

pub(crate) fn compare_relations(a: &RelationChange, b: &RelationChange) -> std::cmp::Ordering {
    (a.relation.as_str(), a.target_kind, a.target_id.as_str()).cmp(&(
        b.relation.as_str(),
        b.target_kind,
        b.target_id.as_str(),
    ))
}

pub(crate) fn compare_findings(a: &Finding, b: &Finding) -> std::cmp::Ordering {
    (
        finding_class(a),
        a.code.as_str(),
        a.subject.id.as_str(),
        first_site_key(a),
    )
        .cmp(&(
            finding_class(b),
            b.code.as_str(),
            b.subject.id.as_str(),
            first_site_key(b),
        ))
}

pub(crate) fn compare_sites(a: &Site, b: &Site) -> std::cmp::Ordering {
    (a.commit, a.path.as_str(), a.line).cmp(&(b.commit, b.path.as_str(), b.line))
}

pub(crate) fn compare_verification_runs(
    a: &VerificationRun,
    b: &VerificationRun,
) -> std::cmp::Ordering {
    (
        a.rule_id.as_str(),
        a.method.as_deref().unwrap_or(""),
        a.status,
        a.commit.as_deref().unwrap_or(""),
        a.binding.as_deref().unwrap_or(""),
    )
        .cmp(&(
            b.rule_id.as_str(),
            b.method.as_deref().unwrap_or(""),
            b.status,
            b.commit.as_deref().unwrap_or(""),
            b.binding.as_deref().unwrap_or(""),
        ))
}

/// Display class of one finding. Changed-intent findings form their own
/// class so intent changes surface before resolved or pre-existing rows.
pub(super) fn finding_class(finding: &Finding) -> u8 {
    if DiagnosticCode::parse(&finding.code) == Some(DiagnosticCode::RequirementStatementChanged) {
        return 2;
    }
    match (finding.comparison, finding.severity) {
        (Comparison::New, Severity::Error) => 0,
        (Comparison::New, Severity::Warning) => 1,
        (Comparison::Resolved, _) => 3,
        (Comparison::PreExisting, _) => 4,
        (Comparison::Uncertain, _) => 5,
    }
}

fn first_site_key(finding: &Finding) -> (u8, String, u32) {
    finding
        .sites
        .first()
        .or_else(|| finding.removed_sites.first())
        .map_or((2, String::new(), 0), |site| {
            (site.commit as u8, site.path.clone(), site.line)
        })
}
