use super::{read_bindings, ScopeId};
use crate::build::read_snapshots;
use crate::envelope::BaselineCompatibility;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::process::Command;

fn git(repo: &Utf8Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn repository() -> (tempfile::TempDir, Utf8PathBuf, String) {
    let dir = tempfile::tempdir().unwrap();
    let repo = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.name", "Test"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(
        &repo,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-qm",
            "base",
        ],
    );
    let base = git(&repo, &["rev-parse", "HEAD"]);
    (dir, repo, base)
}

fn commit_record(repo: &Utf8Path, shard: &str, row: &Value) -> String {
    let path = repo.join(format!(".provenance/state/scopes/default/{shard}"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    // Blank lines must not change the line number in the refusal.
    std::fs::write(path, format!("\n{row}\n")).unwrap();
    git(repo, &["add", "."]);
    git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-qm", "record"],
    );
    git(repo, &["rev-parse", "HEAD"])
}

fn graph_records() -> [(&'static str, Value); 3] {
    [
        (
            "requirements/req.jsonl",
            json!({
                "schema_version": 2, "scope_id": "default", "id": "req_legacy",
                "statement": "The graph is readable.", "status": "resolved"
            }),
        ),
        (
            "rules/rule.jsonl",
            json!({
                "schema_version": 2, "scope_id": "default", "id": "rule_legacy",
                "name": "Readable graph", "statement": "The graph is readable.",
                "status": "active", "severity": "medium", "requirement_ids": ["req_legacy"]
            }),
        ),
        (
            "sources/source.jsonl",
            json!({
                "schema_version": 2, "scope_id": "default", "id": "source_legacy",
                "name": "Graph source", "source_type": "document", "url": null
            }),
        ),
    ]
}

fn assert_migration_refusal(reason: &str, shard: &str, row: &Value, commit: &str) {
    for expected in [
        shard,
        "line 2",
        row["id"].as_str().unwrap(),
        commit,
        "legacy field `retired`",
        "record-deletion migration 026",
    ] {
        assert!(
            reason.contains(expected),
            "missing {expected:?} in {reason:?}"
        );
    }
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn head_graph_records_refuse_every_legacy_retired_field() {
    for (shard, mut row) in graph_records() {
        for retired in [json!(true), json!(false), Value::Null] {
            let (_dir, repo, base) = repository();
            row["retired"] = retired;
            let head = commit_record(&repo, shard, &row);
            let error =
                read_snapshots(&repo, &base, &head, &ScopeId::new("default").unwrap()).unwrap_err();
            assert_migration_refusal(&format!("{error:#}"), shard, &row, &head);
        }
    }
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn head_bindings_refuse_every_legacy_retired_field() {
    let bindings = [
        (
            "verifications/binding.jsonl",
            json!({
                "schema_version": 2, "scope_id": "default", "id": "vb_legacy",
                "rule_id": "rule_legacy", "key": "graph", "method": "examples",
                "declared_by": "fixture", "file": "src/lib.rs"
            }),
        ),
        (
            "implementations/binding.jsonl",
            json!({
                "schema_version": 2, "scope_id": "default", "id": "ib_legacy",
                "rule_id": "rule_legacy", "declared_by": "fixture",
                "file": "src/lib.rs", "symbol": "graph"
            }),
        ),
    ];
    for (shard, mut row) in bindings {
        for retired in [json!(true), json!(false), Value::Null] {
            let (_dir, repo, _base) = repository();
            let clean = commit_record(&repo, shard, &row);
            let (verifications, implementations) =
                read_bindings(&repo, &clean, &ScopeId::new("default").unwrap()).unwrap();
            assert_eq!(verifications.len() + implementations.len(), 1);
            row["retired"] = retired;
            let head = commit_record(&repo, shard, &row);
            let error = read_bindings(&repo, &head, &ScopeId::new("default").unwrap()).unwrap_err();
            assert_migration_refusal(&format!("{error:#}"), shard, &row, &head);
            row.as_object_mut().unwrap().remove("retired");
        }
    }
}

#[test]
#[verifies("rule_report_envelope_states_only_known_facts", examples)]
fn historical_base_keeps_tolerant_graph_parsing() {
    for (shard, mut row) in graph_records() {
        for retired in [json!(true), json!(false), Value::Null] {
            let (_dir, repo, _base) = repository();
            row["retired"] = retired;
            let base = commit_record(&repo, shard, &row);
            row.as_object_mut().unwrap().remove("retired");
            let head = commit_record(&repo, shard, &row);
            let (baseline, reason, base_snapshot, head_snapshot) =
                read_snapshots(&repo, &base, &head, &ScopeId::new("default").unwrap()).unwrap();
            assert_eq!(baseline, BaselineCompatibility::Compatible);
            assert!(reason.is_none());
            assert_eq!(base_snapshot.rules, head_snapshot.rules);
            assert_eq!(base_snapshot.requirements, head_snapshot.requirements);
            assert_eq!(base_snapshot.sources, head_snapshot.sources);
            assert_eq!(
                head_snapshot.rules.len()
                    + head_snapshot.requirements.len()
                    + head_snapshot.sources.len(),
                1
            );
        }
    }
}
