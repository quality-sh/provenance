use super::super::GapKind;
use super::fixtures::*;
use provenance_core::{
    Edge, IdeationTarget, IdeationTargetType, Requirement, ScopeId as Scope, Source,
    SUPPORTED_SCHEMA_VERSION,
};

fn target(kind: IdeationTargetType, id: &str) -> (String, IdeationTarget) {
    (
        "contribution contrib_x".to_string(),
        IdeationTarget {
            artifact_type: kind,
            artifact_id: crate::cache::tests::fixtures::sid(id),
        },
    )
}

type EmptyFamilies = (
    Vec<Source>,
    Vec<Requirement>,
    Vec<provenance_core::Resolution>,
    Vec<provenance_core::Rule>,
    Vec<provenance_core::Topic>,
    Vec<provenance_core::Question>,
    Vec<Edge>,
);

fn fixture() -> EmptyFamilies {
    (
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

fn scope() -> Scope {
    Scope::new("default").unwrap()
}

#[test]
fn dangling_ideation_targets_surface_as_typed_gap_items() {
    let (sources, requirements, resolutions, rules, topics, questions, edges) = fixture();
    let targets = vec![
        target(IdeationTargetType::Domain, "domain_missing"),
        target(IdeationTargetType::Question, "question_missing"),
    ];
    let gaps = compute_for_full(&FixtureFamilies {
        sources: &sources,
        domains: &[],
        boundaries: &[],
        requirements: &requirements,
        resolutions: &resolutions,
        rules: &rules,
        topics: &topics,
        questions: &questions,
        edges: &edges,
        ideation_targets: &targets,
    });
    let dangling = count_kind(&gaps, GapKind::DanglingReference);
    assert_eq!(dangling, 2, "both unresolvable targets are gaps: {gaps:?}");
    assert!(gaps.iter().any(|gap| gap.node_id == "domain_missing"));
}

#[test]
fn resolved_ideation_targets_are_not_gaps() {
    let domain = provenance_core::Domain {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("domain_payroll"),
        name: "Payroll".into(),
        description: None,
        color: None,
    };
    let (sources, requirements, resolutions, rules, topics, questions, edges) = fixture();
    let targets = vec![target(IdeationTargetType::Domain, "domain_payroll")];
    let gaps = compute_for_full(&FixtureFamilies {
        sources: &sources,
        domains: &[domain],
        boundaries: &[],
        requirements: &requirements,
        resolutions: &resolutions,
        rules: &rules,
        topics: &topics,
        questions: &questions,
        edges: &edges,
        ideation_targets: &targets,
    });
    assert_eq!(count_kind(&gaps, GapKind::DanglingReference), 0);
}
