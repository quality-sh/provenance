//! The record count that `import` prints, in each output form.

use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

/// A scope export with one record of each graph type and one verification binding.
const RECORDS: usize = 4;

fn document() -> Value {
    json!({
        "scope": "default",
        "sources": [{
            "scope_id": "default",
            "id": "source_policy",
            "name": "Policy",
            "source_type": "policy",
            "url": null
        }],
        "requirements": [{
            "scope_id": "default",
            "id": "req_parent",
            "statement": "The system keeps the record",
            "status": "discovery"
        }],
        "resolutions": [],
        "rules": [{
            "scope_id": "default",
            "id": "rule_anchor",
            "statement": "The system keeps the record",
            "status": "draft",
            "severity": "medium",
            "requirement_ids": ["req_parent"]
        }],
        "verification_bindings": [{
            "scope_id": "default",
            "id": "verification_anchor",
            "rule_id": "rule_anchor",
            "key": "check",
            "method": "examples",
            "declared_by": "ci://test",
            "file": "tests/check.rs"
        }],
        "threads": [],
        "messages": []
    })
}

#[test]
#[verifies("rule_import_reports_record_count", examples)]
fn import_text_output_counts_each_record_of_the_document() {
    let dir = tempfile::tempdir().unwrap();
    let (repo, input) = repository_with_document(dir.path());

    assert_eq!(
        import(&repo, &input, &[]),
        format!("Imported {RECORDS} records.\n")
    );
    assert_eq!(
        import(&repo, &input, &["--dry-run"]),
        format!(
            "The import of {RECORDS} records passes its checks. The dry run changed nothing.\n"
        )
    );
}

#[test]
#[verifies("rule_import_reports_record_count", examples)]
fn import_json_output_counts_each_record_of_the_document() {
    let dir = tempfile::tempdir().unwrap();
    let (repo, input) = repository_with_document(dir.path());

    for (flags, dry_run) in [(&["--dry-run"][..], true), (&[][..], false)] {
        let flags = [flags, &["--format", "json"][..]].concat();
        let output: Value = serde_json::from_str(&import(&repo, &input, &flags)).unwrap();
        assert_eq!(output, json!({"status": "ok", "dry_run": dry_run, "records": RECORDS}));
    }
}

/// A new repository of the scope `default` and the document file next to it.
fn repository_with_document(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let repo = dir.join("repo");
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["init", "--path", repo.to_str().unwrap(), "--scope", "default"])
        .assert()
        .success();
    let input = dir.join("scope.json");
    std::fs::write(&input, document().to_string()).unwrap();
    (repo, input)
}

/// The standard output of a successful import of `input` with the extra `flags`.
fn import(repo: &Path, input: &Path, flags: &[&str]) -> String {
    let assert = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "import",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--input",
            input.to_str().unwrap(),
        ])
        .args(flags)
        .assert()
        .success();
    String::from_utf8(assert.get_output().stdout.clone()).unwrap()
}
