use camino::Utf8Path;
use provenance_macros::verifies;
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
            "--allow-empty",
            "-qm",
            "Record graph state",
        ],
    );
    git(repo, &["rev-parse", "HEAD"])
}

fn report(shard: &str, initial: Value, final_records: &[Value]) -> String {
    report_with_base(shard, initial, final_records, false)
}

fn report_with_base(
    shard: &str,
    initial: Value,
    final_records: &[Value],
    existing: bool,
) -> String {
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
    let empty = commit(repo);
    write(repo, shard, &[initial]);
    let populated = commit(repo);
    let base = if existing { populated } else { empty };
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
#[verifies("rule_branch_report_head_lifecycle", examples)]
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
        rendered.contains("| added | rule `rule_history` | [archived] The old rule holds. | adds"),
        "{rendered}"
    );
}

#[test]
#[verifies("rule_branch_report_head_lifecycle", examples)]
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
    assert!(rendered.contains("| added | requirement `req_history` | [superseded by 'req_replacement'] The old requirement holds. |"), "{rendered}");
}

#[test]
#[verifies("rule_branch_report_head_lifecycle", examples)]
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
            "| added | resolution `res_history` | [abandoned] Keep the old policy | adds"
        ),
        "{rendered}"
    );
}

#[test]
#[verifies("rule_branch_report_head_lifecycle", examples)]
fn current_added_record_has_no_lifecycle_label() {
    let row = json!({
        "schema_version": 2, "scope_id": "default", "id": "req_current",
        "statement": "The current requirement holds.", "status": "active"
    });
    let rendered = report("requirements/req.jsonl", row.clone(), &[row]);
    assert!(
        rendered
            .contains("| added | requirement `req_current` | The current requirement holds. |  |"),
        "{rendered}"
    );
}

#[test]
#[verifies("rule_branch_report_head_lifecycle", examples)]
fn changed_status_is_labelled_and_kept_in_details() {
    let initial = json!({
        "schema_version": 2, "scope_id": "default", "id": "res_history",
        "title": "Keep the old policy", "position": "Keep the old rule.",
        "rationale": "The old rule applies.", "status": "draft", "requirement_ids": ["req_anchor"]
    });
    let mut row = initial.clone();
    row["status"] = json!("abandoned");
    let rendered = report_with_base("resolutions/res.jsonl", initial, &[row], true);
    assert!(
        rendered.contains(
            "| changed | resolution `res_history` | [abandoned] Keep the old policy → Keep the old policy | status: draft → abandoned |\n"
        ),
        "{rendered}"
    );
    assert_eq!(rendered.matches("[abandoned]").count(), 1, "{rendered}");
}

#[test]
#[verifies("rule_branch_report_head_lifecycle", examples)]
fn sources_and_resolutions_list_every_replacement_in_order() {
    for (shard, kind, initial) in [
        (
            "sources/source.jsonl",
            "source",
            json!({
                "schema_version": 2, "scope_id": "default", "id": "history",
                "name": "Old source", "source_type": "policy"
            }),
        ),
        (
            "resolutions/res.jsonl",
            "resolution",
            json!({
                "schema_version": 2, "scope_id": "default", "id": "history",
                "title": "Old decision", "position": "Keep the old rule.",
                "rationale": "The old rule applies.", "status": "draft", "requirement_ids": ["req_anchor"]
            }),
        ),
    ] {
        let mut second = initial.clone();
        second["id"] = json!("replacement_b");
        second["supersedes"] = json!(["history"]);
        let mut first = second.clone();
        first["id"] = json!("replacement_a");
        let rendered = report(shard, initial.clone(), &[second, initial, first]);
        assert!(
            rendered.contains(&format!(
                "| {kind} `history` | [superseded by 'replacement_a', 'replacement_b']"
            )),
            "{rendered}"
        );
    }
}

#[test]
#[verifies("rule_branch_report_head_lifecycle", examples)]
fn added_deprecated_rule_is_labelled_before_its_statement() {
    let row = json!({
        "schema_version": 2, "scope_id": "default", "id": "rule_history",
        "statement": "The old rule holds.", "status": "deprecated", "severity": "medium",
        "requirement_ids": ["req_anchor"]
    });
    let rendered = report("rules/rule.jsonl", row.clone(), &[row]);
    assert!(
        rendered.contains("| rule `rule_history` | [deprecated] The old rule holds."),
        "{rendered}"
    );
}
