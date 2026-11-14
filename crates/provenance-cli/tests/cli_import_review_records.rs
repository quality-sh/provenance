//! The Requirement review records that a scope import and a typed-spec apply write when they
//! change the statement of a Requirement.

use crate::support::import_reviews::{
    apply_spec, document, import, import_with, patch_statement, reason, reasons, repository,
    requirement, reviews, rule, state_files, AFTER, BEFORE,
};
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[test]
#[verifies("rule_review_record_per_rule", examples)]
fn an_import_that_restates_a_requirement_writes_the_records_that_the_api_writes() {
    let dir = tempfile::tempdir().unwrap();
    let imported = repository(dir.path(), "imported");
    import(&imported, &document(BEFORE, &["rule_a", "rule_b"])).success();
    let start = now_millis();
    import(&imported, &document(AFTER, &["rule_a", "rule_b"])).success();
    let end = now_millis();
    let patched = repository(dir.path(), "patched");
    import(&patched, &document(BEFORE, &["rule_a", "rule_b"])).success();
    patch_statement(&patched, AFTER);

    let records = reviews(&imported);
    assert!(
        records
            .iter()
            .all(|record| (start..=end).contains(&record["changed_at"].as_i64().unwrap())),
        "{records:?}"
    );
    assert_eq!(
        reasons(&records),
        BTreeSet::from([
            reason("rule_a", BEFORE, AFTER),
            reason("rule_b", BEFORE, AFTER),
        ])
    );
    assert_eq!(without_time(records), without_time(reviews(&patched)));
}

#[test]
#[verifies("rule_review_record_per_rule", examples)]
fn an_import_that_restates_a_requirement_writes_a_record_only_for_the_rules_that_cite_it_after() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path(), "repo");
    import(&repo, &document(BEFORE, &["rule_a", "rule_b"])).success();

    import(&repo, &document(AFTER, &["rule_a", "rule_c"])).success();

    assert_eq!(
        reasons(&reviews(&repo)),
        BTreeSet::from([
            reason("rule_a", BEFORE, AFTER),
            reason("rule_c", BEFORE, AFTER),
        ])
    );
}

#[test]
#[verifies("rule_review_record_per_rule", examples)]
fn an_import_that_restates_a_requirement_writes_no_record_for_a_rule_that_it_moves_away() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path(), "repo");
    let mut stored = document(BEFORE, &["rule_a", "rule_b"]);
    with_other_requirement(&mut stored);
    import(&repo, &stored).success();
    let mut moved = document(AFTER, &["rule_a"]);
    with_other_requirement(&mut moved);
    push(&mut moved, "rules", rule("rule_b", "req_other"));

    import(&repo, &moved).success();

    assert_eq!(
        reasons(&reviews(&repo)),
        BTreeSet::from([reason("rule_a", BEFORE, AFTER)])
    );
}

#[test]
#[verifies("rule_review_record_per_rule", examples)]
fn an_import_that_changes_no_statement_writes_no_record() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path(), "repo");
    let mut described = document(BEFORE, &["rule_a"]);
    import(&repo, &described).success();
    described["requirements"][0]["description"] = json!("The record is the account record.");

    import(&repo, &described).success();

    assert_eq!(reviews(&repo), Vec::<Value>::new());
}

#[test]
#[verifies("rule_review_record_per_rule", examples)]
fn an_import_writes_no_record_for_a_requirement_without_a_rule() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path(), "repo");
    import(&repo, &document(BEFORE, &[])).success();

    import(&repo, &document(AFTER, &[])).success();

    assert_eq!(reviews(&repo), Vec::<Value>::new());
}

#[test]
#[verifies("rule_review_record_per_rule", examples)]
fn an_import_writes_no_record_for_a_new_requirement() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path(), "repo");
    import(&repo, &document(BEFORE, &["rule_a"])).success();
    let mut added = document(BEFORE, &["rule_a"]);
    push(&mut added, "requirements", requirement("req_new", AFTER));
    push(&mut added, "rules", rule("rule_new", "req_new"));

    import(&repo, &added).success();

    assert_eq!(reviews(&repo), Vec::<Value>::new());
}

#[test]
#[verifies("rule_review_record_per_rule", examples)]
fn an_import_that_removes_a_requirement_and_its_rules_writes_no_record() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path(), "repo");
    let mut stored = document(BEFORE, &["rule_a", "rule_b"]);
    with_other_requirement(&mut stored);
    import(&repo, &stored).success();
    let mut removed = stored.clone();
    removed["requirements"] = json!([requirement("req_other", BEFORE)]);
    removed["rules"] = json!([]);

    import(&repo, &removed).success();

    assert_eq!(reviews(&repo), Vec::<Value>::new());
}

#[test]
#[verifies("rule_review_record_per_rule", examples)]
fn a_dry_run_of_an_import_that_restates_a_requirement_changes_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path(), "repo");
    import(&repo, &document(BEFORE, &["rule_a"])).success();
    let before = state_files(&repo);

    import_with(&repo, &document(AFTER, &["rule_a"]), &["--dry-run"]).success();

    assert_eq!(state_files(&repo), before);
}

#[test]
#[verifies("rule_review_record_per_rule", examples)]
fn a_typed_spec_apply_that_restates_a_requirement_writes_no_record_for_a_rule_that_it_removes() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path(), "repo");
    apply_spec(&repo, &spec(BEFORE, &["rule_a", "rule_b"]));

    apply_spec(&repo, &spec(AFTER, &["rule_a", "rule_c"]));

    assert_eq!(
        reasons(&reviews(&repo)),
        BTreeSet::from([
            reason("rule_a", BEFORE, AFTER),
            reason("rule_c", BEFORE, AFTER),
        ])
    );
}

/// A typed authoring document with the Requirement `req_record` of `statement` and a Rule of
/// that Requirement for each id of `rules`.
fn spec(statement: &str, rules: &[&str]) -> Value {
    let rules = rules
        .iter()
        .map(|id| json!({"key": id, "id": id, "requirement": "record", "statement": BEFORE}))
        .collect::<Vec<_>>();
    json!({
        "spec": "records",
        "declared_by": "spec://records",
        "requirements": [{"key": "record", "id": "req_record", "statement": statement}],
        "rules": rules
    })
}

/// Adds the Requirement `req_other`, which no Rule cites, to `document`.
fn with_other_requirement(document: &mut Value) {
    push(document, "requirements", requirement("req_other", BEFORE));
}

fn push(document: &mut Value, family: &str, record: Value) {
    document[family].as_array_mut().unwrap().push(record);
}

/// The records without the change time, which comes from the clock of each write.
fn without_time(mut records: Vec<Value>) -> Vec<Value> {
    for record in &mut records {
        record.as_object_mut().unwrap().remove("changed_at");
    }
    records
}

fn now_millis() -> i64 {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    i64::try_from(elapsed.as_millis()).unwrap()
}
