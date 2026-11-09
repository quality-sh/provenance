use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

#[path = "support/review_paths.rs"]
mod review_paths;
#[path = "support/stale_repo.rs"]
mod stale_repo;
use review_paths::provenance;
use stale_repo::{apply_spec, commit, init_repo, write};

fn apply_binding(repo: &Path) {
    let input = json!({
        "spec": "runtime",
        "declared_by": "spec://typescript/runtime",
        "requirements": [{
            "key": "workflows",
            "statement": "Accepted workflows execute"
        }],
        "rules": [{
            "key": "start",
            "id": "rule_start",
            "requirement": "workflows",
            "statement": "Accepted workflows start",
            "implementation": {
                "file": "src/runtime.ts",
                "symbol": "startWorkflow"
            }
        }]
    });
    apply_spec(repo, &input);
}

fn stale(repo: &Path, base: &str, head: &str) -> Vec<Value> {
    let output = provenance()
        .args([
            "api",
            "rules",
            "--query",
            "query=stale",
            "--query",
            &format!("base={base}"),
            "--query",
            &format!("head={head}"),
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice::<Value>(&output).unwrap()["data"]["items"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn changed_typed_implementation_is_touched_without_a_scanner_marker() {
    let directory = init_repo();
    write(
        directory.path(),
        "src/runtime.ts",
        "export function startWorkflow() { return 'started'; }\n",
    );
    apply_binding(directory.path());
    let base = commit(directory.path(), "bind implementation");
    write(
        directory.path(),
        "src/runtime.ts",
        "export function startWorkflow() { return 'running'; }\n",
    );
    let head = commit(directory.path(), "change implementation");

    let report = stale(directory.path(), &base, &head);

    assert_eq!(report.len(), 1);
    assert_eq!(report[0]["kind"], "rule_binding");
    assert_eq!(report[0]["file_path"], "src/runtime.ts");
    assert_eq!(report[0]["state"], "touched");
    assert!(report[0].get("line").is_none());
}

#[test]
#[verifies("rule_missing_file_goes_to_stale_gate", examples)]
fn deleted_typed_implementation_is_gone_without_a_scanner_marker() {
    let directory = init_repo();
    write(
        directory.path(),
        "src/runtime.ts",
        "export function startWorkflow() { return 'started'; }\n",
    );
    apply_binding(directory.path());
    let base = commit(directory.path(), "bind implementation");
    std::fs::remove_file(directory.path().join("src/runtime.ts")).unwrap();
    let head = commit(directory.path(), "delete implementation");

    let report = stale(directory.path(), &base, &head);

    assert_eq!(report.len(), 1);
    assert_eq!(report[0]["kind"], "rule_binding");
    assert_eq!(report[0]["file_path"], "src/runtime.ts");
    assert_eq!(report[0]["state"], "gone");
    let output = provenance()
        .args([
            "export",
            "--repo",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let exported: Value = serde_json::from_slice(&output).unwrap();
    let bindings = exported["implementation_bindings"].as_array().unwrap();
    assert_eq!(bindings.len(), 1, "{exported}");
    assert_eq!(bindings[0]["rule_id"], "rule_start");
    assert_eq!(bindings[0]["file"], "src/runtime.ts");
}

#[test]
fn matching_scanner_and_typed_implementation_is_one_stale_site() {
    let directory = init_repo();
    write(
        directory.path(),
        "src/runtime.ts",
        "// @provenance rule: rule_start\nexport function startWorkflow() { return 'started'; }\n",
    );
    apply_binding(directory.path());
    let base = commit(directory.path(), "bind implementation twice");
    write(
        directory.path(),
        "src/runtime.ts",
        "// @provenance rule: rule_start\nexport function startWorkflow() { return 'running'; }\n",
    );
    let head = commit(directory.path(), "change implementation");

    let report = stale(directory.path(), &base, &head);

    assert_eq!(report.len(), 1);
    assert_eq!(report[0]["subject_id"], "rule_start");
    assert_eq!(report[0]["kind"], "rule_binding");
}

#[test]
fn same_file_different_scanner_symbol_remains_a_distinct_stale_site() {
    let directory = init_repo();
    write(
        directory.path(),
        "src/runtime.ts",
        "// @provenance rule: rule_start\n\
         export function stopWorkflow() { return 'stopped'; }\n\n\
         export function startWorkflow() { return 'started'; }\n",
    );
    apply_binding(directory.path());
    let base = commit(directory.path(), "bind different functions");
    write(
        directory.path(),
        "src/runtime.ts",
        "// @provenance rule: rule_start\n\
         export function stopWorkflow() { return 'halted'; }\n\n\
         export function startWorkflow() { return 'running'; }\n",
    );
    let head = commit(directory.path(), "change both functions");

    let report = stale(directory.path(), &base, &head);

    assert_eq!(report.len(), 2);
    assert!(report
        .iter()
        .all(|site| site["subject_id"] == "rule_start" && site["kind"] == "rule_binding"));
    assert_eq!(
        report
            .iter()
            .filter(|site| site.get("line").is_some())
            .count(),
        1
    );
}
