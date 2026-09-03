mod relation_integrity {
use super::relation_fixtures::{fixture, ids, requirement, rule, sid};
use crate::model::relations::{cycle_in, cycle_refusal, missing_required, reaches, required_refusal};

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
