use super::super::{GapGraph, GraphQuery};
use super::fixtures::*;
use provenance_core::{
    NodeType, Requirement, RequirementStatus, Resolution, Rule, RuleSeverity, ScopeId, Source,
    SourceReference,
};
use std::sync::OnceLock;

fn scope() -> &'static ScopeId {
    static SCOPE: OnceLock<ScopeId> = OnceLock::new();
    SCOPE.get_or_init(|| ScopeId::new("default").unwrap())
}

fn graph_for<'a>(
    sources: &'a [Source],
    requirements: &'a [Requirement],
    resolutions: &'a [Resolution],
    rules: &'a [Rule],
) -> GapGraph<'a> {
    GapGraph {
        scope: scope(),
        sources,
        requirements,
        resolutions,
        rules,
        topics: &[],
        questions: &[],
        threads: &[],
        domains: &[],
        boundaries: &[],
        contributions: &[],
        synthesis_packets: &[],
    }
}

fn sourced(id: &str, source_id: &str) -> Requirement {
    Requirement {
        source_refs: vec![SourceReference {
            source_id: sid(source_id),
            clause: None,
        }],
        ..requirement(id)
    }
}

fn refining(id: &str, parent: &str) -> Requirement {
    Requirement {
        refines: Some(sid(parent)),
        ..requirement(id)
    }
}

fn superseder(id: &str, superseded: &str) -> Requirement {
    Requirement {
        supersedes: vec![sid(superseded)],
        ..requirement(id)
    }
}

fn spawned(id: &str, resolution_id: &str) -> Requirement {
    Requirement {
        spawned_by: Some(sid(resolution_id)),
        ..requirement(id)
    }
}

fn resolving(id: &str, requirements: &[&str]) -> Resolution {
    Resolution {
        requirement_ids: requirements.iter().map(|name| sid(name)).collect(),
        ..resolution(id)
    }
}

fn named_rule(id: &str, requirements: &[&str], resolutions: &[&str]) -> Rule {
    Rule {
        requirement_ids: requirements.iter().map(|name| sid(name)).collect(),
        resolution_ids: resolutions.iter().map(|name| sid(name)).collect(),
        ..rule(id)
    }
}

fn requirement_ids<'a>(records: impl IntoIterator<Item = &'a Requirement>) -> Vec<&'a str> {
    records
        .into_iter()
        .map(|record| record.id.as_str())
        .collect()
}

fn resolution_ids<'a>(records: impl IntoIterator<Item = &'a Resolution>) -> Vec<&'a str> {
    records
        .into_iter()
        .map(|record| record.id.as_str())
        .collect()
}

fn rule_ids<'a>(records: impl IntoIterator<Item = &'a Rule>) -> Vec<&'a str> {
    records
        .into_iter()
        .map(|record| record.id.as_str())
        .collect()
}

#[test]
fn find_locates_records_kind_qualified_and_answers_absence_for_missing_ids() {
    let sources = vec![source("source_a")];
    let requirements = vec![requirement("req_a")];
    let resolutions = vec![resolution("res_a")];
    let rules = vec![rule("rule_a")];
    let graph = graph_for(&sources, &requirements, &resolutions, &rules);
    let query = GraphQuery::new(&graph);

    assert!(query.find(NodeType::Source, "source_a").is_some());
    assert!(query.find(NodeType::Requirement, "req_a").is_some());
    assert!(query.find(NodeType::Resolution, "res_a").is_some());
    assert!(query.find(NodeType::Rule, "rule_a").is_some());
    assert!(query.find(NodeType::Requirement, "req_missing").is_none());
    for kind in [
        NodeType::Source,
        NodeType::Requirement,
        NodeType::Resolution,
        NodeType::Rule,
        NodeType::Topic,
        NodeType::Question,
        NodeType::Domain,
        NodeType::Boundary,
    ] {
        assert!(!query.node_exists(kind, &sid("missing")));
    }
    assert!(query.find_requirement(&sid("req_a")).is_some());
    assert!(query.find_source(&sid("source_a")).is_some());
    assert!(query.find_requirement(&sid("req_missing")).is_none());
}

#[test]
fn a_shared_id_across_kinds_stays_kind_qualified() {
    let requirements = vec![requirement("dup_id")];
    let resolutions = vec![resolving("dup_id", &["req_other"])];
    let rules = vec![named_rule("rule_via", &[], &["dup_id"])];
    let graph = graph_for(&[], &requirements, &resolutions, &rules);
    let query = GraphQuery::new(&graph);

    assert_eq!(query.all(NodeType::Requirement, "dup_id").len(), 1);
    assert_eq!(query.all(NodeType::Resolution, "dup_id").len(), 1);
    assert_eq!(
        rule_ids(query.produced_rules_for_resolution(&sid("dup_id"))),
        vec!["rule_via"]
    );
    // The rule names the resolution `dup_id`, not the requirement
    // `dup_id`, so the requirement produces nothing through that id.
    assert!(query
        .produced_rules_for_requirement(&sid("dup_id"))
        .is_empty());
}

#[test]
fn duplicate_entries_in_one_relation_yield_one_row() {
    let sources = vec![source("source_a")];
    let requirements = vec![
        sourced("req_a", "source_a"),
        superseder("req_s", "req_a"),
        spawned("req_spawned", "res_a"),
    ];
    let resolutions = vec![resolving("res_a", &["req_a", "req_a"])];
    let rules = vec![
        named_rule("rule_dup", &["req_a", "req_a"], &[]),
        named_rule("rule_via_dup", &[], &["res_a", "res_a"]),
    ];
    let graph = graph_for(&sources, &requirements, &resolutions, &rules);
    let query = GraphQuery::new(&graph);

    assert_eq!(
        resolution_ids(query.resolving_resolutions(&sid("req_a"))),
        vec!["res_a"]
    );
    assert_eq!(
        requirement_ids(query.producing_requirements(&sid("rule_dup"))),
        vec!["req_a"]
    );
    assert_eq!(
        rule_ids(query.produced_rules_for_requirement(&sid("req_a"))),
        vec!["rule_dup", "rule_via_dup"]
    );
    assert_eq!(
        requirement_ids(query.requirement_supersedes(&requirements[1])),
        vec!["req_a"]
    );
    assert_eq!(
        requirement_ids(query.requirements_citing_source(&sid("source_a"))),
        vec!["req_a"]
    );
    assert_eq!(
        requirement_ids(query.requirements_spawned_by(&sid("res_a"))),
        vec!["req_spawned"]
    );
}

#[test]
fn duplicate_records_stay_distinct_rows_in_record_order() {
    let twin_first = Requirement {
        status: RequirementStatus::Active,
        ..refining("req_twin", "req_parent")
    };
    let twin_second = Requirement {
        status: RequirementStatus::Resolved,
        ..refining("req_twin", "req_parent")
    };
    let requirements = vec![refining("req_parent", "req_root"), twin_first, twin_second];
    let rules = vec![
        Rule {
            severity: RuleSeverity::Low,
            ..named_rule("rule_twin", &["req_parent"], &[])
        },
        Rule {
            severity: RuleSeverity::Critical,
            ..named_rule("rule_twin", &["req_parent"], &[])
        },
    ];
    let graph = graph_for(&[], &requirements, &[], &rules);
    let query = GraphQuery::new(&graph);

    let children = query.children_of(&sid("req_parent"));
    assert_eq!(children.len(), 2, "both twin records answer the join");
    assert_eq!(children[0].status, RequirementStatus::Active);
    assert_eq!(children[1].status, RequirementStatus::Resolved);

    let produced = query.produced_rules_for_requirement(&sid("req_parent"));
    assert_eq!(produced.len(), 2, "both twin rule records answer the join");
    assert_eq!(produced[0].severity, RuleSeverity::Low);
    assert_eq!(produced[1].severity, RuleSeverity::Critical);

    // The inverse attribution names the requirement once, the way the
    // forward traversal names each requirement once: both twin rules
    // name `req_parent`, and the attribution lists it once.
    assert_eq!(
        requirement_ids(query.requirements_behind_rule(&sid("rule_twin")).to_vec()),
        vec!["req_parent"]
    );
}

#[test]
fn joins_follow_record_order_not_id_order() {
    let sources = vec![source("source_a")];
    let requirements = vec![
        sourced("req_zulu", "source_a"),
        sourced("req_alpha", "source_a"),
        sourced("req_mike", "source_a"),
    ];
    let resolutions = vec![
        resolving("res_zulu", &["req_zulu", "req_mike"]),
        resolving("res_alpha", &["req_alpha"]),
    ];
    let rules = vec![
        named_rule("rule_zulu", &["req_mike"], &[]),
        named_rule("rule_alpha", &["req_zulu"], &[]),
    ];
    let graph = graph_for(&sources, &requirements, &resolutions, &rules);
    let query = GraphQuery::new(&graph);

    assert_eq!(
        resolution_ids(query.resolving_resolutions(&sid("req_mike"))),
        vec!["res_zulu"],
        "one row per naming resolution, in resolution record order"
    );
    assert_eq!(
        requirement_ids(query.requirements_citing_source(&sid("source_a"))),
        vec!["req_zulu", "req_alpha", "req_mike"]
    );
    assert_eq!(
        requirement_ids(query.requirements_resolved_by(&resolutions[0])),
        vec!["req_zulu", "req_mike"],
        "the resolves row follows requirement record order"
    );
    assert_eq!(
        rule_ids(query.produced_rules_for_requirement(&sid("req_zulu"))),
        vec!["rule_alpha"]
    );
}

#[test]
fn produced_rules_direct_and_through_resolution_are_one_row() {
    let requirements = vec![requirement("req_a")];
    let resolutions = vec![resolving("res_a", &["req_a"])];
    let rules = vec![named_rule("rule_both", &["req_a"], &["res_a"])];
    let graph = graph_for(&[], &requirements, &resolutions, &rules);
    let query = GraphQuery::new(&graph);

    assert_eq!(
        rule_ids(query.produced_rules_for_requirement(&sid("req_a"))),
        vec!["rule_both"]
    );
    assert_eq!(
        rule_ids(query.produced_rules_for_resolution(&sid("res_a"))),
        vec!["rule_both"]
    );
}

#[test]
fn reverse_supersession_prefers_lowest_id_not_record_order() {
    let requirements = vec![
        superseder("req_zulu", "req_old"),
        superseder("req_alpha", "req_old"),
    ];
    let graph = graph_for(&[], &requirements, &[], &[]);
    let query = GraphQuery::new(&graph);

    assert_eq!(
        query
            .requirement_superseded_by(&sid("req_old"))
            .map(|record| record.id.as_str()),
        Some("req_alpha")
    );
    assert!(query
        .requirement_superseded_by(&sid("req_unsuperseded"))
        .is_none());
}

#[test]
fn attribution_inverts_the_forward_traversal() {
    let requirements = vec![requirement("req_a"), requirement("req_b")];
    let resolutions = vec![resolving("res_a", &["req_a"])];
    let rules = vec![
        named_rule("rule_direct", &["req_a"], &[]),
        named_rule("rule_via", &[], &["res_a"]),
        named_rule("rule_b", &["req_b"], &[]),
        rule("rule_unproduced"),
    ];
    let graph = graph_for(&[], &requirements, &resolutions, &rules);
    let query = GraphQuery::new(&graph);

    let behind =
        |rule_id: &str| requirement_ids(query.requirements_behind_rule(&sid(rule_id)).to_vec());
    assert_eq!(behind("rule_direct"), vec!["req_a"]);
    assert_eq!(behind("rule_via"), vec!["req_a"]);
    assert_eq!(behind("rule_b"), vec!["req_b"]);
    assert!(behind("rule_unproduced").is_empty());
    assert!(behind("rule_missing").is_empty());

    // Every forward pair reads back through the inverse attribution.
    for requirement in &requirements {
        for produced in query.produced_rules_for_requirement(&requirement.id) {
            assert!(
                query
                    .requirements_behind_rule(&produced.id)
                    .contains(&requirement),
                "rule {} forgot requirement {}",
                produced.id.as_str(),
                requirement.id.as_str()
            );
        }
    }
}

#[test]
fn resolves_spawned_and_parent_reads_come_from_the_index() {
    let requirements = vec![
        requirement("req_a"),
        spawned("req_spawned", "res_a"),
        spawned("req_orphan_spawn", "res_missing"),
        refining("req_child", "req_parent"),
        requirement("req_parent"),
    ];
    let resolutions = vec![resolving("res_a", &["req_a", "req_missing"])];
    let graph = graph_for(&[], &requirements, &resolutions, &[]);
    let query = GraphQuery::new(&graph);

    assert_eq!(
        requirement_ids(query.requirements_resolved_by(&resolutions[0])),
        vec!["req_a"],
        "a resolution resolves only requirements the scope holds"
    );
    assert_eq!(
        requirement_ids(query.requirements_spawned_by(&sid("res_a"))),
        vec!["req_spawned"]
    );
    assert_eq!(
        requirement_ids(query.requirements_spawned_by(&sid("res_missing"))),
        vec!["req_orphan_spawn"],
        "the inverse join reads the field, not the resolution's existence"
    );
    assert_eq!(
        requirement_ids(query.children_of(&sid("req_parent"))),
        vec!["req_child"]
    );
    assert!(query.children_of(&sid("req_missing_parent")).is_empty());
    assert_eq!(
        query
            .refines_parent(&sid("req_child"))
            .map(|parent| parent.id.as_str()),
        Some("req_parent")
    );
    assert!(query.refines_parent(&sid("req_a")).is_none());
    assert!(query.refines_parent(&sid("req_missing")).is_none());
}

#[test]
fn forward_supersedes_and_depends_on_list_existing_records_in_record_order() {
    let requirements = vec![
        superseder("req_mike", "req_zulu"),
        superseder("req_alpha", "req_zulu"),
        requirement("req_zulu"),
        refining("req_dep", "req_root"),
    ];
    let mut dependent = requirement("req_dependent");
    dependent.depends_on = vec![sid("req_dep"), sid("req_missing")];
    let requirements = [requirements, vec![dependent]].concat();
    let graph = graph_for(&[], &requirements, &[], &[]);
    let query = GraphQuery::new(&graph);

    assert_eq!(
        requirement_ids(query.requirement_supersedes(&requirements[0])),
        vec!["req_zulu"]
    );
    assert_eq!(
        requirement_ids(query.requirement_depends_on(&requirements[4])),
        vec!["req_dep"],
        "a dangling depends_on entry renders no record"
    );
    assert!(query.requirement_supersedes(&requirements[2]).is_empty());
}

#[test]
fn source_supersession_reads_through_the_index() {
    let mut successor = source("source_zulu");
    successor.supersedes = vec![sid("source_old")];
    let mut other = source("source_alpha");
    other.supersedes = vec![sid("source_old")];
    let sources = vec![successor, other, source("source_old")];
    let graph = graph_for(&sources, &[], &[], &[]);
    let query = GraphQuery::new(&graph);

    assert_eq!(
        query
            .source_superseded_by(&sid("source_old"))
            .map(|record| record.id.as_str()),
        Some("source_alpha")
    );
    assert!(query.source_superseded_by(&sid("source_alpha")).is_none());
}
