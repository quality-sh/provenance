//! One dangling target per relation class, planted by hand where the
//! writers would refuse: a single field, a list, a citation, a link, and a
//! thread parent. Gaps report each one by its declaration, and check
//! refuses the scope.

use assert_cmd::Command;
use predicates::str::contains;
use std::path::Path;

const REQUIREMENTS: &str = r#"{"schema_version":1,"scope_id":"default","id":"req_root","statement":"The root requirement","status":"active","source_refs":[{"source_id":"source_missing","clause":null}]}
{"schema_version":1,"scope_id":"default","id":"req_child","statement":"The child requirement","status":"active","refines":"req_missing"}"#;

const RULES: &str = r#"{"schema_version":1,"scope_id":"default","id":"rule_root","statement":"The root rule","status":"active","severity":"high","requirement_ids":["req_root"],"resolution_ids":["res_missing"]}"#;

const TOPICS: &str = r#"{"schema_version":1,"scope_id":"default","id":"topic_root","requirement_id":"req_root","title":"Root topic","status":"explored","links":[{"target_type":"rule","target_id":"rule_missing"}]}"#;

const THREADS: &str = r#"{"schema_version":1,"scope_id":"default","id":"thread_orphan","parent":{"node_type":"requirement","node_id":"req_vanished"},"status":"active","created_at":1}"#;

fn planted_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path());
    let scope = dir.path().join(".provenance/state/scopes/default");
    write_jsonl(&scope.join("requirements/req.jsonl"), REQUIREMENTS);
    write_jsonl(&scope.join("rules/rule.jsonl"), RULES);
    write_jsonl(&scope.join("topics/topic.jsonl"), TOPICS);
    write_jsonl(&scope.join("threads/threads.jsonl"), THREADS);
    dir
}

#[test]
fn gaps_name_every_dangling_target_by_its_declaration() {
    let dir = planted_repo();

    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "gaps",
            "--repo",
            dir.path().to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let gaps: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    let dangling: Vec<(String, String, String)> = gaps
        .iter()
        .filter(|gap| gap["kind"] == "dangling_reference")
        .map(|gap| {
            (
                gap["node_type"].as_str().unwrap().to_string(),
                gap["node_id"].as_str().unwrap().to_string(),
                gap["reason"].as_str().unwrap().to_string(),
            )
        })
        .collect();

    let row =
        |kind: &str, id: &str, reason: &str| (kind.to_string(), id.to_string(), reason.to_string());
    assert_eq!(
        dangling,
        vec![
            row(
                "requirement",
                "req_root",
                "cites points at missing source source_missing"
            ),
            row(
                "requirement",
                "req_child",
                "refines points at missing requirement req_missing"
            ),
            row(
                "rule",
                "rule_root",
                "resolution_ids points at missing resolution res_missing"
            ),
            row(
                "topic",
                "topic_root",
                "links points at missing rule rule_missing"
            ),
            row(
                "requirement",
                "req_vanished",
                "thread thread_orphan points at missing requirement req_vanished"
            ),
        ]
    );
}

#[test]
fn check_refuses_every_dangling_target() {
    let dir = planted_repo();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "check",
            "--repo",
            dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(contains(
            "requirement req_child has dangling reference: refines requirement req_missing",
        ))
        .stderr(contains(
            "requirement req_root has dangling reference: cites source source_missing",
        ))
        .stderr(contains(
            "rule rule_root has dangling reference: resolution_ids resolution res_missing",
        ))
        .stderr(contains(
            "topic topic_root has dangling reference: link rule rule_missing",
        ))
        .stderr(contains(
            "thread thread_orphan has dangling reference: parent requirement req_vanished",
        ));
}

fn init(repo: &Path) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
}

fn write_jsonl(path: &Path, records: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("{records}\n")).unwrap();
}
