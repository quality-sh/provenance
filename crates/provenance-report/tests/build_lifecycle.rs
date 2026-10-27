//! Implementation aids: no Rule defines lifecycle labels in branch reports.

use camino::Utf8Path;
use provenance_report::{
    build::{build_envelope, BuildInput},
    render_envelope, RenderFormat,
};
use serde_json::{json, Value};
use std::{fmt::Write as _, path::Path, process::Command};

fn git(repo: &Path, args: &[&str]) -> String {
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

fn write(repo: &Path, shard: &str, records: &[Value]) {
    let path = repo.join(format!(".provenance/state/scopes/default/{shard}"));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut text = String::new();
    for row in records {
        writeln!(text, "{row}").unwrap();
    }
    std::fs::write(path, text).unwrap();
}

fn commit(repo: &Path) -> String {
    git(repo, &["add", "."]);
    git(
        repo,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "Record graph state",
        ],
    );
    git(repo, &["rev-parse", "HEAD"])
}

fn report(shard: &str, initial: Value, final_records: &[Value]) -> String {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q"]);
    git(repo, &["config", "user.name", "Test"]);
    git(repo, &["config", "user.email", "test@example.com"]);
    write(
        repo,
        "requirements/req.jsonl",
        &[json!({
            "schema_version": 2, "scope_id": "default", "id": "req_anchor",
            "statement": "The graph is readable.", "status": "active"
        })],
    );
    let base = commit(repo);
    write(repo, shard, &[initial]);
    commit(repo);
    write(repo, shard, final_records);
    let head = commit(repo);
    let repo = Utf8Path::from_path(repo).unwrap();
    let envelope = build_envelope(&BuildInput {
        repo,
        scan_path: repo,
        scope: "default",
        base: &base,
        head: &head,
        repository: "quality-sh/provenance",
    })
    .unwrap();
    render_envelope(
        &serde_json::to_string(&envelope).unwrap(),
        RenderFormat::Markdown,
    )
    .unwrap()
}

#[test]
fn added_rule_archived_in_range_has_head_status() {
    let mut row = json!({
        "schema_version": 2, "scope_id": "default", "id": "rule_history",
        "statement": "The old rule holds.", "status": "active", "severity": "medium",
        "requirement_ids": ["req_anchor"]
    });
    let initial = row.clone();
    row["status"] = json!("archived");
    row["archived_in_commit"] = json!({"commit": "a".repeat(40), "at": "2026-09-13T00:00:00Z"});
    let rendered = report("rules/rule.jsonl", initial, &[row]);
    assert!(
        rendered.contains(
            "| added | rule `rule_history` | The old rule holds. | status at head: archived;"
        ),
        "{rendered}"
    );
}

#[test]
fn requirement_superseded_in_range_has_head_status() {
    let initial = json!({
        "schema_version": 2, "scope_id": "default", "id": "req_history",
        "statement": "The old requirement holds.", "status": "active"
    });
    let mut old = initial.clone();
    old["status"] = json!("resolved");
    let replacement = json!({
        "schema_version": 2, "scope_id": "default", "id": "req_replacement",
        "statement": "The new requirement holds.", "status": "active", "supersedes": ["req_history"]
    });
    let rendered = report("requirements/req.jsonl", initial, &[old, replacement]);
    assert!(rendered.contains("| added | requirement `req_history` | The old requirement holds. | status at head: resolved |"), "{rendered}");
}

#[test]
fn added_resolution_abandoned_in_range_has_head_status() {
    let initial = json!({
        "schema_version": 2, "scope_id": "default", "id": "res_history",
        "title": "Keep the old policy", "position": "Keep the old rule.",
        "rationale": "The old rule applies.", "status": "draft", "requirement_ids": ["req_anchor"]
    });
    let mut row = initial.clone();
    row["status"] = json!("abandoned");
    let rendered = report("resolutions/res.jsonl", initial, &[row]);
    assert!(
        rendered.contains(
            "| added | resolution `res_history` | Keep the old policy | status at head: abandoned;"
        ),
        "{rendered}"
    );
}
