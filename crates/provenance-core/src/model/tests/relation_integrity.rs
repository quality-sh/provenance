mod relation_integrity {
use super::relation_fixtures::{fixture, ids, requirement, rule, sid};
use crate::model::relations::{
    cycle_in, cycle_refusal, cycle_with_added_edges, missing_required, reaches, required_refusal,
};

#[test]
fn a_rule_with_no_requirement_is_missing_its_required_relation() {
    let bare = rule("rule_bare", &[], &["res_threshold"]);
    let decl = missing_required(&bare).expect("the requirement list is required");
    assert_eq!(decl.name, "requirement_ids");
    assert_eq!(required_refusal(decl), "a rule needs one requirement");
    assert!(missing_required(&rule("rule_ok", &["req_overtime"], &[])).is_none());
}

#[test]
fn every_fixture_record_carries_its_required_relations() {
    let records = fixture();
    assert!(records.rules.iter().all(|rule| missing_required(rule).is_none()));
    assert!(records
        .resolutions
        .iter()
        .all(|resolution| missing_required(resolution).is_none()));
    assert!(records
        .requirements
        .iter()
        .all(|requirement| missing_required(requirement).is_none()));
}

#[test]
fn a_refinement_chain_that_returns_to_its_start_is_a_cycle() {
    let mut records = fixture();
    assert_eq!(cycle_in(&records.requirements, "refines"), None);
    records.requirements[0].refines = Some(sid("req_penalty"));
    let cycle = cycle_in(&records.requirements, "refines").unwrap();
    assert_eq!(cycle.closes_from, sid("req_penalty"));
    assert_eq!(cycle.closes_into, sid("req_overtime"));
    assert_eq!(cycle_refusal("refines", &cycle), "refines forms a cycle: req_overtime -> req_penalty -> req_overtime");
}

/// The pair that closes a cycle is the last hop back to the start, not
/// the first hop out of it.
#[test]
fn a_three_record_cycle_is_named_by_its_closing_pair_with_the_cycle_printed() {
    let mut a = requirement("req_cyc_a", None, &[]);
    let mut b = requirement("req_cyc_b", None, &[]);
    let mut c = requirement("req_cyc_c", None, &[]);
    a.refines = Some(sid("req_cyc_b"));
    b.refines = Some(sid("req_cyc_c"));
    c.refines = Some(sid("req_cyc_a"));
    let records = vec![a, b, c];

    let cycle = cycle_in(&records, "refines").expect("the chain closes");
    assert_eq!(cycle.closes_from, sid("req_cyc_c"));
    assert_eq!(cycle.closes_into, sid("req_cyc_a"));
    assert_eq!(
        cycle_refusal("refines", &cycle),
        "refines forms a cycle: req_cyc_a -> req_cyc_b -> req_cyc_c -> req_cyc_a"
    );
}

#[test]
fn a_hypothetical_edge_reports_the_complete_cycle_path() {
    let mut a = requirement("req_cyc_a", None, &[]);
    let mut b = requirement("req_cyc_b", None, &[]);
    let c = requirement("req_cyc_c", None, &[]);
    a.depends_on = ids(&["req_cyc_b"]);
    b.depends_on = ids(&["req_cyc_c"]);
    let records = vec![a, b, c];

    let cycle = cycle_with_added_edges(
        &records,
        "depends_on",
        &sid("req_cyc_c"),
        &[sid("req_cyc_a")],
    )
    .expect("the added edge closes the chain");
    assert_eq!(cycle.closes_from, sid("req_cyc_c"));
    assert_eq!(cycle.closes_into, sid("req_cyc_a"));
    assert_eq!(
        cycle_refusal("depends_on", &cycle),
        "depends_on forms a cycle: req_cyc_a -> req_cyc_b -> req_cyc_c -> req_cyc_a"
    );
}

#[test]
fn a_batch_walk_reports_the_first_reachable_target_over_a_shared_path() {
    let mut first = requirement("req_first", None, &[]);
    let mut branch_a = requirement("req_branch_a", None, &[]);
    let mut branch_b = requirement("req_branch_b", None, &[]);
    let mut shared = requirement("req_shared", None, &[]);
    let owner = requirement("req_owner", None, &[]);
    let unreachable = requirement("req_unreachable", None, &[]);
    let mut cycle_a = requirement("req_cycle_a", None, &[]);
    let mut cycle_b = requirement("req_cycle_b", None, &[]);
    first.depends_on = ids(&["req_branch_a", "req_branch_b"]);
    branch_a.depends_on = ids(&["req_shared"]);
    branch_b.depends_on = ids(&["req_shared"]);
    shared.depends_on = ids(&["req_owner"]);
    cycle_a.depends_on = ids(&["req_cycle_b"]);
    cycle_b.depends_on = ids(&["req_cycle_a"]);
    let records = vec![
        cycle_b,
        branch_b,
        unreachable,
        first,
        shared,
        owner,
        branch_a,
        cycle_a,
    ];

    let cycle = cycle_with_added_edges(
        &records,
        "depends_on",
        &sid("req_owner"),
        &[
            sid("req_unreachable"),
            sid("req_first"),
            sid("req_branch_b"),
        ],
    )
    .expect("the second target reaches the owner");

    assert_eq!(cycle.closes_from, sid("req_owner"));
    assert_eq!(cycle.closes_into, sid("req_first"));
    assert_eq!(
        cycle_refusal("depends_on", &cycle),
        "depends_on forms a cycle: req_first -> req_branch_a -> req_shared -> req_owner -> req_first"
    );
}

#[test]
fn a_batch_walk_ignores_unreachable_targets_in_a_cyclic_graph() {
    let owner = requirement("req_owner", None, &[]);
    let unreachable = requirement("req_unreachable", None, &[]);
    let mut cycle_a = requirement("req_cycle_a", None, &[]);
    let mut cycle_b = requirement("req_cycle_b", None, &[]);
    cycle_a.depends_on = ids(&["req_cycle_b"]);
    cycle_b.depends_on = ids(&["req_cycle_a"]);

    assert_eq!(
        cycle_with_added_edges(
            &[owner, unreachable, cycle_a, cycle_b],
            "depends_on",
            &sid("req_owner"),
            &[sid("req_unreachable"), sid("req_cycle_a")],
        ),
        None
    );
}

#[test]
fn a_batch_walk_reports_a_self_edge_as_the_first_cycle() {
    let records = vec![
        requirement("req_owner", None, &[]),
        requirement("req_unreachable", None, &[]),
    ];

    let cycle = cycle_with_added_edges(
        &records,
        "depends_on",
        &sid("req_owner"),
        &[sid("req_unreachable"), sid("req_owner")],
    )
    .expect("the self edge closes a cycle");

    assert_eq!(cycle.closes_from, sid("req_owner"));
    assert_eq!(cycle.closes_into, sid("req_owner"));
    assert_eq!(cycle.path, ids(&["req_owner", "req_owner"]));
}

#[test]
fn a_chain_over_one_relation_ignores_the_others() {
    let mut records = fixture();
    records.requirements[0].depends_on = ids(&["req_penalty"]);
    let cycle = cycle_in(&records.requirements, "depends_on").unwrap();
    assert_eq!(cycle.closes_from, sid("req_penalty"));
    assert_eq!(cycle.closes_into, sid("req_overtime"));
    assert_eq!(cycle_in(&records.requirements, "refines"), None);
    assert!(reaches(&records.requirements, "depends_on", &sid("req_penalty"), &sid("req_overtime")));
    assert!(!reaches(&records.requirements, "refines", &sid("req_overtime"), &sid("req_penalty")));
}
}
