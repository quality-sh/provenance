//! The Requirement review records that a scope import and a typed-spec apply write when they
//! change the statement of a Requirement.

use crate::cli_ste_statement_paths::support::provenance_tree;
use crate::support::import_reviews::{
    apply_spec, document, import, import_with, patch_statement, reason, reasons, refusal,
    repository, requirement, requirement_etag, reviews, rule, typed_spec, AFTER, BEFORE,
};
use crate::support::review_paths::provenance;
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

/// Implementation aid with no Rule binding: no Rule states that a dry run of an import changes
/// no file. The test shows that the review records of an import are written in the stage that a
/// dry run discards.
#[test]
fn a_dry_run_of_an_import_that_restates_a_requirement_changes_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path(), "repo");
    import(&repo, &document(BEFORE, &["rule_a"])).success();
    let before = provenance_tree(&repo);

    import_with(&repo, &document(AFTER, &["rule_a"]), &["--dry-run"]).success();

    assert_eq!(provenance_tree(&repo), before);
}

#[test]
#[verifies("rule_review_record_per_rule", examples)]
fn a_typed_spec_apply_that_restates_a_requirement_writes_no_record_for_a_rule_that_it_removes() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path(), "repo");
    apply_spec(
        &repo,
        &typed_spec(BEFORE, &[("rule_a", "record"), ("rule_b", "record")]),
    );

    apply_spec(
        &repo,
        &typed_spec(AFTER, &[("rule_a", "record"), ("rule_c", "record")]),
    );

    assert_eq!(
        reasons(&reviews(&repo)),
        BTreeSet::from([
            reason("rule_a", BEFORE, AFTER),
            reason("rule_c", BEFORE, AFTER),
        ])
    );
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

/// Implementation aid with no Rule binding: it shows which error wins when a Requirement change
/// through the API meets a `rules.jsonl` that does not parse, and that no file changes.
#[test]
fn a_requirement_change_through_the_api_with_an_unreadable_rule_file_names_the_rule_file() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path(), "repo");
    import(&repo, &document(BEFORE, &["rule_a"])).success();
    let etag = requirement_etag(&repo);
    let rules = repo.join(".provenance/state/scopes/default/rules/rule.jsonl");
    let mut text = std::fs::read_to_string(&rules).unwrap();
    text.push_str("{\"broken\n");
    std::fs::write(&rules, text).unwrap();
    let before = provenance_tree(&repo);

    for change in [
        json!({"actor": "agent", "statement": AFTER}),
        json!({"actor": "agent", "description": "The record is the account record."}),
    ] {
        let refused = refusal(
            provenance()
                .args(["api", "requirements/req_record", "--method", "patch"])
                .args(["--header", &format!("If-Match: {etag}")])
                .args(["--input", "-", "--repo", repo.to_str().unwrap()])
                .args(["--scope", "default", "--format", "json"])
                .write_stdin(change.to_string())
                .assert(),
        );

        let error: Value = serde_json::from_str(&refused.1).unwrap();
        assert_eq!(refused.0, Some(1), "{change}");
        assert_eq!(error["error"]["kind"], "invalid_state_file", "{change}");
        assert_eq!(error["error"]["file"], "scopes/default/rules/rule.jsonl");
        assert_eq!(error["error"]["line"], 2, "{change}");
        assert_eq!(provenance_tree(&repo), before, "{change}");
    }
}
