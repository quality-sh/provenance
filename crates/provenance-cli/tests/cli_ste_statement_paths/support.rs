use assert_cmd::Command;
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};

pub const REQUIREMENTS_SHARD: &str = ".provenance/state/scopes/default/requirements/req.jsonl";
pub const RULES_SHARD: &str = ".provenance/state/scopes/default/rules/rule.jsonl";

pub fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

pub fn init(repo: &Path) {
    provenance()
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
    create_requirement(repo, "req_rule_anchor", "The anchor requirement holds");
}

pub fn create_requirement(repo: &Path, id: &str, statement: &str) {
    provenance()
        .args([
            "requirements",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            id,
            "--statement",
            statement,
        ])
        .assert()
        .success();
}

pub fn create_rule(repo: &Path, id: &str, statement: &str) {
    provenance()
        .args([
            "rules",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            id,
            "--requirement-id",
            "req_rule_anchor",
            "--statement",
            statement,
        ])
        .assert()
        .success();
}

pub fn export(repo: &Path, output: &Path) -> Value {
    provenance()
        .args([
            "export",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap()
}

pub fn write_json(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

pub fn diagnostic(kind: &str, id: &str, start: usize) -> Value {
    json!({
        "resource_kind": kind,
        "scope_id": "default",
        "id": id,
        "field": "statement",
        "standard": "ASD-STE100",
        "issue": 9,
        "analyzer_version": "0.2.2",
        "rule": "8.1",
        "disposition": "violation",
        "span": {"start": start, "end": start + 1},
        "message": "Do not use semicolons in descriptive text."
    })
}

pub fn error_json(output: &std::process::Output) -> Value {
    let stderr = String::from_utf8(output.stderr.clone()).unwrap();
    serde_json::from_str(
        stderr
            .trim()
            .strip_prefix("Error: ")
            .expect("CLI failure contains one machine-readable JSON object"),
    )
    .unwrap()
}

pub fn provenance_tree(repo: &Path) -> BTreeMap<String, Vec<u8>> {
    let root = repo.join(".provenance");
    let mut files = BTreeMap::new();
    collect_files(&root, &root, &mut files);
    files
}

fn collect_files(root: &Path, current: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
    let mut entries = std::fs::read_dir(current)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            files.insert(
                format!("{}/", path.strip_prefix(root).unwrap().to_string_lossy()),
                Vec::new(),
            );
            collect_files(root, &path, files);
        } else {
            files.insert(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                std::fs::read(path).unwrap(),
            );
        }
    }
}

pub fn record(id: &str, statement: &str, kind: &str) -> String {
    let value = if kind == "requirement" {
        json!({
            "schema_version": 1,
            "scope_id": "default",
            "id": id,
            "statement": statement,
            "status": "active"
        })
    } else {
        json!({
            "schema_version": 1,
            "scope_id": "default",
            "id": id,
            "statement": statement,
            "status": "active",
            "severity": "high",
            "requirement_ids": ["req_anchor"]
        })
    };
    format!("{}\n", serde_json::to_string(&value).unwrap())
}

pub fn git(repo: &Path, arguments: &[&str]) -> std::process::Output {
    let output = std::process::Command::new("git")
        .current_dir(repo)
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
