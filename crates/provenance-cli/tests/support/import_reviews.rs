//! Repositories and reads for the tests of the Requirement review records that a scope import
//! writes and of the refusal of an import or an export of a scope that holds review state.

use super::review_paths::provenance;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const BEFORE: &str = "The system keeps the record.";
pub const AFTER: &str = "The system keeps the record for one year.";

/// A scope export with the Requirement `req_record` of `statement` and a Rule of that
/// Requirement for each id of `rules`.
pub fn document(statement: &str, rules: &[&str]) -> Value {
    let rules = rules
        .iter()
        .map(|id| rule(id, "req_record"))
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

pub fn requirement(id: &str, statement: &str) -> Value {
    json!({"scope_id": "default", "id": id, "statement": statement, "status": "discovery"})
}

pub fn rule(id: &str, requirement: &str) -> Value {
    json!({
        "scope_id": "default",
        "id": id,
        "statement": BEFORE,
        "status": "draft",
        "severity": "medium",
        "requirement_ids": [requirement]
    })
}

/// A new repository `name` below `dir` with the scope `default`.
pub fn repository(dir: &Path, name: &str) -> PathBuf {
    let repo = dir.join(name);
    provenance()
        .args(["init", "--path", repo.to_str().unwrap()])
        .args(["--scope", "default"])
        .args(["--disposition-actor-id", "reviewer"])
        .assert()
        .success();
    repo
}

/// A repository whose last import changed the statement of `req_record`, which the Rules
/// `rule_a` and `rule_b` cite.
pub fn restated_repository(dir: &Path) -> PathBuf {
    let repo = repository(dir, "restated");
    import(&repo, &document(BEFORE, &["rule_a", "rule_b"])).success();
    import(&repo, &document(AFTER, &["rule_a", "rule_b"])).success();
    repo
}

/// The import of `document` into the scope `default` of `repo`.
pub fn import(repo: &Path, document: &Value) -> assert_cmd::assert::Assert {
    import_with(repo, document, &[])
}

/// The import of `document` into the scope `default` of `repo`, with the flags `extra`.
pub fn import_with(repo: &Path, document: &Value, extra: &[&str]) -> assert_cmd::assert::Assert {
    let input = repo.with_extension("import.json");
    std::fs::write(&input, document.to_string()).unwrap();
    provenance()
        .args(["import", "--repo", repo.to_str().unwrap()])
        .args(["--scope", "default", "--input", input.to_str().unwrap()])
        .args(extra)
        .assert()
}

/// The export of the scope `default` of `repo`.
pub fn export(repo: &Path) -> assert_cmd::assert::Assert {
    provenance()
        .args(["export", "--repo", repo.to_str().unwrap()])
        .args(["--scope", "default", "--format", "json"])
        .assert()
}

/// Applies the typed authoring document `spec` to the scope `default` of `repo`.
pub fn apply_spec(repo: &Path, spec: &Value) {
    provenance()
        .args([
            "api",
            "authoring-changes",
            "--method",
            "post",
            "--input",
            "-",
        ])
        .args(["--repo", repo.to_str().unwrap()])
        .args(["--scope", "default", "--format", "json"])
        .write_stdin(spec.to_string())
        .assert()
        .success();
}

/// A typed authoring document with the Requirement `req_record` (key `record`) of `statement`,
/// the Requirement `req_other` (key `other`), and one Rule for each pair of `rules`: the id
/// and the key of the Requirement that the Rule cites.
pub fn typed_spec(statement: &str, rules: &[(&str, &str)]) -> Value {
    let rules = rules
        .iter()
        .map(|(id, requirement)| {
            json!({"key": id, "id": id, "requirement": requirement, "statement": BEFORE})
        })
        .collect::<Vec<_>>();
    json!({
        "spec": "records",
        "declared_by": "spec://records",
        "requirements": [
            {"key": "record", "id": "req_record", "statement": statement},
            {"key": "other", "id": "req_other", "statement": BEFORE}
        ],
        "rules": rules
    })
}

/// Changes the statement of `req_record` through the API.
pub fn patch_statement(repo: &Path, statement: &str) {
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

/// Begins a verification run for `rule`, which clears the open review records of that Rule.
pub fn verify(repo: &Path, rule: &str) {
    std::fs::create_dir_all(repo.join("tests")).unwrap();
    std::fs::write(repo.join("tests/record.rs"), "// exercises the record\n").unwrap();
    let input = json!({
        "rule": rule,
        "key": format!("{rule}_examples"),
        "method": "examples",
        "declared_by": "ci://tests",
        "file": "tests/record.rs"
    });
    provenance()
        .args(["api", "verification-runs/begin-verification"])
        .args(["--method", "post", "--input", "-"])
        .args(["--repo", repo.to_str().unwrap()])
        .args(["--scope", "default", "--format", "json"])
        .write_stdin(input.to_string())
        .assert()
        .success();
}

/// The Requirement review records of the scope `default`, in file order.
pub fn reviews(repo: &Path) -> Vec<Value> {
    let path = repo.join(".provenance/state/scopes/default/requirements/review.jsonl");
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// The Rule, the field and the two statements of each record.
pub fn reasons(records: &[Value]) -> BTreeSet<(String, String, String, String)> {
    records
        .iter()
        .map(|record| {
            assert_eq!(record["requirement_id"], "req_record", "{record}");
            let text = |field: &str| record[field].as_str().unwrap().to_owned();
            (
                text("rule_id"),
                text("field"),
                text("before"),
                text("after"),
            )
        })
        .collect()
}

pub fn reason(rule: &str, before: &str, after: &str) -> (String, String, String, String) {
    let text = str::to_owned;
    (text(rule), text("statement"), text(before), text(after))
}

/// The exit code and the standard error of a refused command, which writes no standard output.
pub fn refusal(assert: assert_cmd::assert::Assert) -> (Option<i32>, String) {
    let output = assert.failure().get_output().clone();
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "");
    (
        output.status.code(),
        String::from_utf8(output.stderr).unwrap(),
    )
}
