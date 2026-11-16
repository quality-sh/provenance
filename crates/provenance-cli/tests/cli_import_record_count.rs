//! The record count that `import` prints, in each output form.

#[path = "support/modern_lifecycle.rs"]
mod modern_lifecycle;
use crate::thread_record;

use assert_cmd::Command;
use modern_lifecycle::add_modern_lifecycle;
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use thread_record::thread_in;

/// The number of records in the document.
const RECORDS: usize = 4;

/// The record arrays of a scope export.
const FAMILIES: [&str; 15] = [
    "sources",
    "requirements",
    "topics",
    "questions",
    "resolutions",
    "rules",
    "verification_bindings",
    "implementation_bindings",
    "threads",
    "messages",
    "contributions",
    "synthesis_packets",
    "proposal_cards",
    "assertion_records",
    "dispositions",
];

/// A scope export with a Source, a Requirement, a Rule and a verification binding.
fn document() -> Value {
    json!({
        "scope": "default",
        "sources": [{
            "scope_id": "default",
            "id": "source_a",
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
    let (repo, input) = repository_with_document(dir.path(), &document());

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
    let (repo, input) = repository_with_document(dir.path(), &document());

    for (flags, dry_run) in [(&["--dry-run"][..], true), (&[][..], false)] {
        let flags = [flags, &["--format", "json"][..]].concat();
        let stdout = import(&repo, &input, &flags);
        let output: Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(
            output,
            json!({"status": "ok", "dry_run": dry_run, "records": RECORDS})
        );
    }
}

/// The document with one record or more in each record array: the records of [`document`],
/// a Topic, a Question, a Resolution, an implementation binding, a thread, a message and the
/// records of one proposal lifecycle. Each record has an id that no other record has.
fn document_with_each_family() -> Value {
    let mut document = document();
    document["topics"] = json!([{
        "scope_id": "default",
        "id": "topic_a",
        "requirement_id": "req_parent",
        "title": "Record keeping",
        "status": "open"
    }]);
    document["questions"] = json!([{
        "scope_id": "default",
        "id": "question_a",
        "topic_id": "topic_a",
        "requirement_id": "req_parent",
        "question": "How long does the system keep the record?",
        "resolution_method": "ask",
        "status": "open"
    }]);
    document["resolutions"] = json!([{
        "scope_id": "default",
        "id": "res_a",
        "title": "Record keeping",
        "position": "Keep the record",
        "rationale": "The policy asks for the record",
        "status": "draft",
        "requirement_ids": ["req_parent"]
    }]);
    document["implementation_bindings"] = json!([{
        "scope_id": "default",
        "id": "implementation_anchor",
        "rule_id": "rule_anchor",
        "declared_by": "spec://typescript/workflows",
        "file": "src/runtime.ts",
        "symbol": "startWorkflow"
    }]);
    document["threads"] = json!([thread_in("thread_a", "default", "req_parent", 1)]);
    document["messages"] = json!([{
        "scope_id": "default",
        "id": "message_a",
        "thread_id": "thread_a",
        "role": "user",
        "body": "The policy asks for the record.",
        "created_at": 1
    }]);
    add_modern_lifecycle(&mut document);
    document
}

/// The record array of each record id in `document`.
fn family_of_each_id(document: &Value) -> BTreeMap<String, &'static str> {
    let mut families = BTreeMap::new();
    for family in FAMILIES {
        for record in document[family].as_array().unwrap() {
            let id = record["id"].as_str().unwrap().to_owned();
            assert_eq!(families.insert(id, family), None, "{record}");
        }
    }
    families
}

/// An import of a document with records in each record array gives as its count the number of
/// records that an export of the scope then gives, and that export has records of each array.
#[test]
#[verifies("rule_import_reports_record_count", conformance)]
fn import_count_equals_the_exported_records_of_each_family() {
    let dir = tempfile::tempdir().unwrap();
    let document = document_with_each_family();
    let (repo, input) = repository_with_document(dir.path(), &document);

    let stdout = import(&repo, &input, &["--format", "json"]);
    let output: Value = serde_json::from_str(&stdout).unwrap();
    let exported = export_jsonl(&repo);
    let family_of = family_of_each_id(&document);
    let families = exported
        .lines()
        .map(|line| {
            let record: Value = serde_json::from_str(line).unwrap();
            family_of[record["id"].as_str().unwrap()]
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(
        (output["records"].clone(), families),
        (json!(exported.lines().count()), BTreeSet::from(FAMILIES))
    );
}

/// A new repository of the scope `default`, with the disposition actor `reviewer`, and the file
/// of `document` next to it.
fn repository_with_document(dir: &Path, document: &Value) -> (PathBuf, PathBuf) {
    let repo = dir.join("repo");
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--disposition-actor-id",
            "reviewer",
        ])
        .assert()
        .success();
    let input = dir.join("scope.json");
    std::fs::write(&input, document.to_string()).unwrap();
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

/// The JSON Lines export of the scope `default`: one line for each record.
fn export_jsonl(repo: &Path) -> String {
    let assert = Command::cargo_bin("provenance")
        .unwrap()
        .args(["export", "--repo", repo.to_str().unwrap()])
        .args(["--scope", "default", "--format", "jsonl"])
        .assert()
        .success();
    String::from_utf8(assert.get_output().stdout.clone()).unwrap()
}
