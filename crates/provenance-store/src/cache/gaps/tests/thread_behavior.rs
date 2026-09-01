use super::super::{compute_gaps, GapGraph, GapItem, GapKind};
use super::fixtures::{domain, requirement, thread};
use provenance_core::{NodeType, ScopeId};

fn gaps_with(
    domains: &[provenance_core::Domain],
    threads: &[provenance_core::Thread],
) -> Vec<GapItem> {
    let scope = ScopeId::new("default").unwrap();
    compute_gaps(&GapGraph {
        scope: &scope,
        sources: &[],
        requirements: &[requirement("req_overtime")],
        resolutions: &[],
        rules: &[],
        topics: &[],
        questions: &[],
        edges: &[],
        threads,
        domains,
        boundaries: &[],
    })
}

#[test]
fn a_thread_on_an_existing_domain_is_not_a_dangling_reference() {
    let gaps = gaps_with(
        &[domain("domain_payroll")],
        &[thread("thread_a", NodeType::Domain, "domain_payroll")],
    );
    assert!(
        !gaps
            .iter()
            .any(|gap| gap.kind == GapKind::DanglingReference && gap.node_id == "domain_payroll"),
        "an existing domain must not be reported missing: {gaps:?}"
    );
}

#[test]
fn a_thread_on_a_missing_domain_is_a_dangling_reference() {
    let gaps = gaps_with(&[], &[thread("thread_a", NodeType::Domain, "domain_gone")]);
    assert!(
        gaps.iter()
            .any(|gap| gap.kind == GapKind::DanglingReference && gap.node_id == "domain_gone"),
        "a missing domain behind a thread must be reported: {gaps:?}"
    );
}
