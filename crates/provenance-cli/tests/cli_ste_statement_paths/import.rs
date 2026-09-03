use super::support::{
    create_requirement, create_rule, diagnostic, error_json, export, init, provenance,
    provenance_tree, write_json, REQUIREMENTS_SHARD,
};
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

fn import_output(repo: &Path, input: &Path, dry_run: bool) -> std::process::Output {
    let mut command = provenance();
    command.args([
        "import",
        "--repo",
        repo.to_str().unwrap(),
        "--scope",
        "default",
        "--input",
        input.to_str().unwrap(),
        "--format",
        "json",
    ]);
    if dry_run {
        command.arg("--dry-run");
    }
    command.output().unwrap()
}

fn candidate(repo: &Path, path: &Path, statement: &str) {
    let exported_path = path.with_extension("base.json");
    let mut value = export(repo, &exported_path);
    for requirement in value["requirements"].as_array_mut().unwrap() {
        if requirement["id"] == "req_changed" {
            requirement["statement"] = json!(statement);
        }
    }
    value["rules"] = json!([{
        "schema_version": 1,
        "scope_id": "default",
        "id": "rule_changed",
        "statement": statement,
        "status": "active",
        "severity": "high",
        "requirement_ids": ["req_changed"]
    }, {
        "schema_version": 1,
        "scope_id": "default",
        "id": "rule_added",
        "statement": statement,
        "status": "active",
        "severity": "high",
        "requirement_ids": ["req_changed"]
    }]);
    write_json(path, &value);
}

#[test]
#[verifies("rule_ste_import_changed_statement_gate", examples)]
fn apply_and_dry_run_reject_with_identical_diagnostics_and_preserve_all_state_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().join("repo");
    init(&repo);
    create_requirement(&repo, "req_changed", "Original statement");
    create_rule(&repo, "rule_changed", "Original statement");
    let input = directory.path().join("candidate.json");
    candidate(&repo, &input, "Café; changed");
    let before = provenance_tree(&repo);

    let dry_run = import_output(&repo, &input, true);
    assert!(!dry_run.status.success());
    assert_eq!(provenance_tree(&repo), before);
    let apply = import_output(&repo, &input, false);
    assert!(!apply.status.success());
    assert_eq!(provenance_tree(&repo), before);

    let dry_report = error_json(&dry_run);
    let apply_report = error_json(&apply);
    assert_eq!(apply_report, dry_report);
    assert_eq!(apply_report["error"], "asd_ste100_violations");
    assert_eq!(
        apply_report["diagnostics"],
        Value::Array(vec![
            diagnostic("requirement", "req_changed", 5),
            diagnostic("rule", "rule_added", 5),
            diagnostic("rule", "rule_changed", 5),
        ])
    );
}

#[test]
#[verifies("rule_ste_import_changed_statement_gate", examples)]
fn import_allows_clean_changes_and_unchanged_legacy_invalid_statements() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().join("repo");
    init(&repo);
    create_requirement(&repo, "req_changed", "Legacy statement");
    let requirement_path = repo.join(REQUIREMENTS_SHARD);
    let legacy = std::fs::read_to_string(&requirement_path)
        .unwrap()
        .replace("Legacy statement", "Legacy; invalid");
    std::fs::write(requirement_path, legacy).unwrap();
    let input = directory.path().join("candidate.json");
    let mut value = export(&repo, &directory.path().join("legacy.json"));
    value["requirements"][0]["description"] = json!("Unrelated metadata");
    value["rules"] = json!([{
        "schema_version": 1,
        "scope_id": "default",
        "id": "rule_clean",
        "statement": "A clean rule",
        "status": "active",
        "severity": "high",
        "requirement_ids": [value["requirements"][0]["id"].clone()]
    }]);
    write_json(&input, &value);

    let dry_run = import_output(&repo, &input, true);
    assert!(
        dry_run.status.success(),
        "{}",
        String::from_utf8_lossy(&dry_run.stderr)
    );
    let apply = import_output(&repo, &input, false);
    assert!(
        apply.status.success(),
        "{}",
        String::from_utf8_lossy(&apply.stderr)
    );
}
