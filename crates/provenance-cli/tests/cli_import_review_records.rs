//! The Requirement review records that a scope import writes, and the refusal of an import or
//! an export of a scope that has one.

use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const BEFORE: &str = "The system keeps the record.";
const AFTER: &str = "The system keeps the record for one year.";
const REFUSAL: &str = "Error: scope `default` has Requirement review records, which an import \
                       or an export cannot carry; read and change the records of this scope \
                       through the API\n";

/// A scope export with the Requirement `req_record` of `statement` and a Rule of that
/// Requirement for each id of `rules`.
fn document(statement: &str, rules: &[&str]) -> Value {
    let rules = rules
        .iter()
        .map(|id| {
            json!({
                "scope_id": "default",
                "id": id,
                "statement": "The system keeps the record.",
                "status": "draft",
                "severity": "medium",
                "requirement_ids": ["req_record"]
            })
        })
        .collect::<Vec<_>>();
    json!({
        "scope": "default",
        "sources": [],
        "requirements": [requirement("req_record", statement)],
        "resolutions": [],
        "rules": rules,
        "threads": [],
        "messages": []
    })
}

fn requirement(id: &str, statement: &str) -> Value {
    json!({"scope_id": "default", "id": id, "statement": statement, "status": "discovery"})
}

/// A repository whose last import changed the statement of `req_record`, which the Rules
/// `rule_a` and `rule_b` cite.
fn restated_repository(dir: &Path) -> PathBuf {
    let repo = repository(dir, "restated");
    import(&repo, &document(BEFORE, &["rule_a", "rule_b"])).success();
    import(&repo, &document(AFTER, &["rule_a", "rule_b"])).success();
    repo
}

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
fn an_import_that_restates_a_requirement_and_changes_its_rules_writes_a_record_for_each_rule() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path(), "repo");
    import(&repo, &document(BEFORE, &["rule_a", "rule_b"])).success();

    import(&repo, &document(AFTER, &["rule_a", "rule_c"])).success();

    assert_eq!(
        reasons(&reviews(&repo)),
        BTreeSet::from([
            reason("rule_a", BEFORE, AFTER),
            reason("rule_b", BEFORE, AFTER),
            reason("rule_c", BEFORE, AFTER),
        ])
    );
}

#[test]
#[verifies("rule_review_record_per_rule", examples)]
fn an_import_that_changes_no_statement_writes_no_record_and_a_later_import_passes() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path(), "repo");
    let mut described = document(BEFORE, &["rule_a"]);
    import(&repo, &described).success();
    described["requirements"][0]["description"] = json!("The record is the account record.");

    import(&repo, &described).success();

    assert_eq!(reviews(&repo), Vec::<Value>::new());
    import(&repo, &document(BEFORE, &["rule_a"])).success();
}

#[test]
#[verifies("rule_review_record_per_rule", examples)]
fn an_import_writes_no_record_for_a_requirement_without_a_rule_or_a_new_requirement() {
    let dir = tempfile::tempdir().unwrap();
    let repo = repository(dir.path(), "repo");
    import(&repo, &document(BEFORE, &[])).success();
    let mut changed = document(AFTER, &[]);
    changed["requirements"]
        .as_array_mut()
        .unwrap()
        .push(requirement("req_new", BEFORE));

    import(&repo, &changed).success();

    assert_eq!(reviews(&repo), Vec::<Value>::new());
}

#[test]
#[verifies("rule_import_export_refuse_requirement_reviews", examples)]
fn an_import_on_a_scope_with_a_requirement_review_record_is_refused_and_changes_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let repo = restated_repository(dir.path());
    let before = state_files(&repo);

    let output = import(&repo, &document(BEFORE, &["rule_a", "rule_b"]))
        .failure()
        .get_output()
        .clone();

    let code = output.status.code();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!((code, stderr), (Some(1), REFUSAL.to_owned()));
    assert_eq!(state_files(&repo), before);
}

#[test]
#[verifies("rule_import_export_refuse_requirement_reviews", examples)]
fn an_export_of_a_scope_with_a_requirement_review_record_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let repo = restated_repository(dir.path());

    let output = provenance()
        .args(["export", "--repo", repo.to_str().unwrap()])
        .args(["--scope", "default", "--format", "json"])
        .assert()
        .failure()
        .get_output()
        .clone();

    let code = output.status.code();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        (code, stdout, stderr),
        (Some(1), String::new(), REFUSAL.to_owned())
    );
}

fn provenance() -> Command {
    Command::cargo_bin("provenance").unwrap()
}

/// A new repository `name` below `dir` with the scope `default`.
fn repository(dir: &Path, name: &str) -> PathBuf {
    let repo = dir.join(name);
    provenance()
        .args(["init", "--path", repo.to_str().unwrap()])
        .args(["--scope", "default"])
        .args(["--disposition-actor-id", "reviewer"])
        .assert()
        .success();
    repo
}

/// The import of `document` into the scope `default` of `repo`.
fn import(repo: &Path, document: &Value) -> assert_cmd::assert::Assert {
    let input = repo.with_extension("import.json");
    std::fs::write(&input, document.to_string()).unwrap();
    provenance()
        .args(["import", "--repo", repo.to_str().unwrap()])
        .args(["--scope", "default", "--input", input.to_str().unwrap()])
        .assert()
}

/// Changes the statement of `req_record` through the API.
fn patch_statement(repo: &Path, statement: &str) {
    let repo = repo.to_str().unwrap();
    let read = provenance()
        .args(["req_record", "get", "--format", "json", "--repo", repo])
        .assert()
        .success();
    let read: Value = serde_json::from_slice(&read.get_output().stdout).unwrap();
    let etag = read["edit"]["etag"].as_str().unwrap();
    provenance()
        .args(["api", "requirements/req_record", "--method", "patch"])
        .args(["--header", &format!("If-Match: {etag}")])
        .args(["--input", "-"])
        .args(["--repo", repo, "--scope", "default", "--format", "json"])
        .write_stdin(json!({"actor": "agent", "statement": statement}).to_string())
        .assert()
        .success();
}

/// The Requirement review records of the scope `default`, in file order.
fn reviews(repo: &Path) -> Vec<Value> {
    let path = repo.join(".provenance/state/scopes/default/requirements/review.jsonl");
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// The Rule, the field and the two statements of each record.
fn reasons(records: &[Value]) -> BTreeSet<(String, String, String, String)> {
    records
        .iter()
        .map(|record| {
            assert_eq!(record["requirement_id"], "req_record", "{record}");
            let text = |field: &str| record[field].as_str().unwrap().to_owned();
            (text("rule_id"), text("field"), text("before"), text("after"))
        })
        .collect()
}

fn reason(rule: &str, before: &str, after: &str) -> (String, String, String, String) {
    let text = str::to_owned;
    (text(rule), text("statement"), text(before), text(after))
}

/// The records without the change time, which comes from the clock of each write.
fn without_time(mut records: Vec<Value>) -> Vec<Value> {
    for record in &mut records {
        record.as_object_mut().unwrap().remove("changed_at");
    }
    records
}

/// The bytes of each file below the state folder of `repo`, by relative path.
fn state_files(repo: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let root = repo.join(".provenance/state");
    let mut files = BTreeMap::new();
    let mut folders = vec![root.clone()];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(folder).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                folders.push(path);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                files.insert(path.strip_prefix(&root).unwrap().to_owned(), bytes);
            }
        }
    }
    files
}

fn now_millis() -> i64 {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    i64::try_from(elapsed.as_millis()).unwrap()
}
