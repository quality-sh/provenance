use super::super::{compute_gaps, GapGraph, GapKind, GraphQuery};
use super::fixtures::*;
use provenance_core::{
    Requirement, RequirementStatus, Resolution, ResolutionStatus, Rule, RuleStatus, ScopeId,
    Source, SourceReference,
};
use std::sync::OnceLock;

/// The PR #45 homepage scale in miniature: 228 requirements, 165
/// decisions, 576 rules, and 7 sources with the same relation shape the
/// generated fixture uses. The index serves lookups from per-id buckets,
/// so the totals below are exact, and a sweep over every record answers
/// from the buckets rather than from rescanning the record vectors.
const REQUIREMENTS: usize = 228;
const RESOLUTIONS: usize = 165;
const RULES: usize = 576;
const SOURCES: usize = 7;
const UNFINISHED: usize = 42;

fn scope() -> &'static ScopeId {
    static SCOPE: OnceLock<ScopeId> = OnceLock::new();
    SCOPE.get_or_init(|| ScopeId::new("pr-45-scale").unwrap())
}

fn requirement_id(number: usize) -> String {
    format!("req_{number:03}")
}

#[test]
fn lookups_answer_the_pr_45_scale_with_exact_bucket_totals() {
    let sources: Vec<Source> = (0..SOURCES)
        .map(|number| source(&format!("source_{number:02}")))
        .collect();
    let requirements: Vec<Requirement> = (0..REQUIREMENTS)
        .map(|number| {
            let mut record = Requirement {
                status: RequirementStatus::Active,
                ..requirement(&requirement_id(number))
            };
            if number >= UNFINISHED {
                record.source_refs = vec![SourceReference {
                    source_id: sid(&format!("source_{:02}", number % SOURCES)),
                    clause: None,
                }];
            }
            record
        })
        .collect();
    let resolutions: Vec<Resolution> = (0..RESOLUTIONS)
        .map(|number| Resolution {
            status: ResolutionStatus::Approved,
            requirement_ids: vec![sid(&requirement_id(number))],
            ..resolution(&format!("res_{number:03}"))
        })
        .collect();
    let rules: Vec<Rule> = (0..RULES)
        .map(|number| Rule {
            status: RuleStatus::Active,
            requirement_ids: vec![sid(&requirement_id(number % RESOLUTIONS))],
            resolution_ids: vec![sid(&format!("res_{:03}", number % RESOLUTIONS))],
            ..rule(&format!("rule_{number:03}"))
        })
        .collect();
    let graph = GapGraph {
        scope: scope(),
        sources: &sources,
        requirements: &requirements,
        resolutions: &resolutions,
        rules: &rules,
        topics: &[],
        questions: &[],
        threads: &[],
        domains: &[],
        boundaries: &[],
        contributions: &[],
        synthesis_packets: &[],
    };
    let query = GraphQuery::new(&graph);

    let produced_total: usize = (0..REQUIREMENTS)
        .map(|number| {
            query
                .produced_rules_for_requirement(&sid(&requirement_id(number)))
                .len()
        })
        .sum();
    assert_eq!(
        produced_total, RULES,
        "every rule names exactly one requirement"
    );

    let attributed_total: usize = (0..RULES)
        .map(|number| {
            query
                .requirements_behind_rule(&sid(&format!("rule_{number:03}")))
                .len()
        })
        .sum();
    assert_eq!(
        attributed_total, RULES,
        "the inverse attribution names one requirement per rule"
    );

    let resolved_total: usize = resolutions
        .iter()
        .map(|resolution| query.requirements_resolved_by(resolution).len())
        .sum();
    assert_eq!(resolved_total, RESOLUTIONS);

    let cited_total: usize = sources
        .iter()
        .map(|source| query.requirements_citing_source(&source.id).len())
        .sum();
    assert_eq!(cited_total, REQUIREMENTS - UNFINISHED);

    assert_eq!(
        query
            .produced_rules_for_requirement(&sid("req_missing"))
            .len(),
        0
    );
    assert!(query.find_requirement(&sid("req_missing")).is_none());
}

#[test]
fn lookups_are_unaffected_by_unrelated_volume() {
    let sources: Vec<Source> = (0..SOURCES)
        .map(|number| source(&format!("source_{number:02}")))
        .collect();
    let mut requirements: Vec<Requirement> = (0..REQUIREMENTS)
        .map(|number| {
            let mut record = Requirement {
                status: RequirementStatus::Active,
                ..requirement(&requirement_id(number))
            };
            if number >= UNFINISHED {
                record.source_refs = vec![SourceReference {
                    source_id: sid(&format!("source_{:02}", number % SOURCES)),
                    clause: None,
                }];
            }
            record
        })
        .collect();
    // Unrelated records pad every record vector without adding relations.
    requirements.extend((0..5_000).map(|number| requirement(&format!("req_pad_{number:05}"))));
    let resolutions: Vec<Resolution> = (0..RESOLUTIONS)
        .map(|number| Resolution {
            status: ResolutionStatus::Approved,
            requirement_ids: vec![sid(&requirement_id(number))],
            ..resolution(&format!("res_{number:03}"))
        })
        .collect();
    let rules: Vec<Rule> = (0..RULES)
        .map(|number| Rule {
            status: RuleStatus::Active,
            requirement_ids: vec![sid(&requirement_id(number % RESOLUTIONS))],
            resolution_ids: vec![sid(&format!("res_{:03}", number % RESOLUTIONS))],
            ..rule(&format!("rule_{number:03}"))
        })
        .collect();
    let graph = GapGraph {
        scope: scope(),
        sources: &sources,
        requirements: &requirements,
        resolutions: &resolutions,
        rules: &rules,
        topics: &[],
        questions: &[],
        threads: &[],
        domains: &[],
        boundaries: &[],
        contributions: &[],
        synthesis_packets: &[],
    };
    let query = GraphQuery::new(&graph);

    // The padded records share no relations, so every bucket keeps its
    // exact membership: the sweep totals are the same as the unpadded
    // scale above.
    let produced_total: usize = (0..REQUIREMENTS)
        .map(|number| {
            query
                .produced_rules_for_requirement(&sid(&requirement_id(number)))
                .len()
        })
        .sum();
    assert_eq!(produced_total, RULES);
    assert!(query.find_requirement(&sid("req_pad_00000")).is_some());
    assert_eq!(
        query.producing_requirements(&sid("rule_000")).len(),
        1,
        "a padded record must not join a rule it is unrelated to"
    );
}

#[test]
fn gap_policy_reports_the_same_scale_counts_through_the_index() {
    let sources: Vec<Source> = (0..SOURCES)
        .map(|number| source(&format!("source_{number:02}")))
        .collect();
    let requirements: Vec<Requirement> = (0..REQUIREMENTS)
        .map(|number| {
            let mut record = Requirement {
                status: RequirementStatus::Active,
                ..requirement(&requirement_id(number))
            };
            if number >= UNFINISHED {
                record.source_refs = vec![SourceReference {
                    source_id: sid(&format!("source_{:02}", number % SOURCES)),
                    clause: None,
                }];
            }
            record
        })
        .collect();
    let resolutions: Vec<Resolution> = (0..RESOLUTIONS)
        .map(|number| Resolution {
            status: ResolutionStatus::Approved,
            requirement_ids: vec![sid(&requirement_id(number))],
            ..resolution(&format!("res_{number:03}"))
        })
        .collect();
    let rules: Vec<Rule> = (0..RULES)
        .map(|number| Rule {
            status: RuleStatus::Active,
            requirement_ids: vec![sid(&requirement_id(number % RESOLUTIONS))],
            resolution_ids: vec![sid(&format!("res_{:03}", number % RESOLUTIONS))],
            ..rule(&format!("rule_{number:03}"))
        })
        .collect();
    let gaps = compute_gaps(&GapGraph {
        scope: scope(),
        sources: &sources,
        requirements: &requirements,
        resolutions: &resolutions,
        rules: &rules,
        topics: &[],
        questions: &[],
        threads: &[],
        domains: &[],
        boundaries: &[],
        contributions: &[],
        synthesis_packets: &[],
    });

    let count = |kind: GapKind| gaps.iter().filter(|gap| gap.kind == kind).count();
    assert_eq!(count(GapKind::MissingSourceRefs), UNFINISHED);
    assert_eq!(count(GapKind::MissingDomainId), REQUIREMENTS);
    assert_eq!(count(GapKind::NoResolvingDecision), 0);
    assert_eq!(count(GapKind::NoProducedRules), 0);
    assert_eq!(count(GapKind::UnreferencedSource), 0);
    assert_eq!(count(GapKind::DanglingReference), 0);
    assert_eq!(gaps.len(), UNFINISHED + REQUIREMENTS);
}
