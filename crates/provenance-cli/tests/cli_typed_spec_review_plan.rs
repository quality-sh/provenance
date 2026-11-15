//! The typed-spec plan asks a review of the evidence of exactly the Rules for which the apply of
//! the same spec writes a Requirement review record.

use crate::support::import_reviews::{apply_spec, repository, reviews, typed_spec, AFTER, BEFORE};
use crate::support::review_paths::provenance;
use provenance_macros::verifies;
use serde_json::Value;
use std::collections::BTreeSet;

#[test]
#[verifies("rule_workflowd_plan_explains_impact", examples)]
fn a_typed_spec_plan_asks_no_review_for_a_rule_that_the_restatement_deletes() {
    let stored = typed_spec(BEFORE, &[("rule_a", "record"), ("rule_b", "record")]);
    let changed = typed_spec(AFTER, &[("rule_a", "record"), ("rule_c", "record")]);

    let (planned, written) = plan_then_apply(&stored, &changed);

    assert_eq!(planned, ids(&["rule_a", "rule_c"]));
    assert_eq!(written, planned);
}

#[test]
#[verifies("rule_workflowd_plan_explains_impact", examples)]
fn a_typed_spec_plan_asks_no_review_for_a_rule_that_the_restatement_moves_away() {
    let stored = typed_spec(BEFORE, &[("rule_a", "record"), ("rule_b", "record")]);
    let changed = typed_spec(AFTER, &[("rule_a", "record"), ("rule_b", "other")]);

    let (planned, written) = plan_then_apply(&stored, &changed);

    assert_eq!(planned, ids(&["rule_a"]));
    assert_eq!(written, planned);
}

#[test]
#[verifies("rule_workflowd_plan_explains_impact", examples)]
fn a_typed_spec_plan_asks_a_review_for_a_rule_that_the_restatement_moves_in() {
    let stored = typed_spec(BEFORE, &[("rule_a", "record"), ("rule_b", "other")]);
    let changed = typed_spec(AFTER, &[("rule_a", "record"), ("rule_b", "record")]);

    let (planned, written) = plan_then_apply(&stored, &changed);

    assert_eq!(planned, ids(&["rule_a", "rule_b"]));
    assert_eq!(written, planned);
}

/// Applies `stored` to a new repository, then plans and applies `changed`. Gives the Rules that
/// the plan marks `review_required` and the Rules of the review records that the apply writes.
fn plan_then_apply(stored: &Value, changed: &Value) -> (BTreeSet<String>, BTreeSet<String>) {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path(), "repo");
    apply_spec(&repo, stored);
    let plan = provenance()
        .args(["api", "authoring-plans", "--method", "post", "--input", "-"])
        .args(["--repo", repo.to_str().unwrap()])
        .args(["--scope", "default", "--format", "json"])
        .write_stdin(changed.to_string())
        .assert()
        .success();
    let plan: Value = serde_json::from_slice(&plan.get_output().stdout).unwrap();
    let planned = plan["data"]["affected_rules"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|rule| rule["evidence"]["review_required"].as_bool().unwrap())
        .map(|rule| rule["id"].as_str().unwrap().to_owned())
        .collect();

    apply_spec(&repo, changed);

    let written = reviews(&repo)
        .iter()
        .map(|record| record["rule_id"].as_str().unwrap().to_owned())
        .collect();
    (planned, written)
}

fn ids(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|id| (*id).to_owned()).collect()
}
