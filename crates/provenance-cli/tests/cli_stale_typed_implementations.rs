use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command as ProcessCommand;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn git(repo: &Path, args: &[&str]) -> String {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn write(repo: &Path, relative: &str, contents: &str) {
    let path = repo.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

fn commit(repo: &Path, message: &str) -> String {
    git(repo, &["add", "."]);
    git(repo, &["commit", "-m", message]);
    git(repo, &["rev-parse", "HEAD"])
}

fn init_repo() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    git(directory.path(), &["init", "-q"]);
    git(
        directory.path(),
        &["config", "user.email", "test@example.com"],
    );
    git(directory.path(), &["config", "user.name", "Test"]);
    provenance()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    directory
}

fn apply_binding(repo: &Path) {
    let target = repo.join("src/runtime.ts");
    let input = json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
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
                "file": target,
                "symbol": "startWorkflow"
            }
        }]
    });
    provenance()
        .args([
            "sdk",
            "apply",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .write_stdin(serde_json::to_vec(&input).unwrap())
        .assert()
        .success();
}

fn stale(repo: &Path, base: &str, head: &str) -> Value {
    let output = provenance()
        .args([
            "stale",
            base,
            head,
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
    serde_json::from_slice(&output).unwrap()
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

    assert_eq!(report["summary"]["total_sites"], 1);
    assert_eq!(report["summary"]["touched"], 1);
    assert_eq!(report["sites"][0]["kind"], "rule_binding");
    assert_eq!(report["sites"][0]["file_path"], "src/runtime.ts");
    assert_eq!(report["sites"][0]["state"], "touched");
    assert!(report["sites"][0].get("line").is_none());
}

#[test]
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

    assert_eq!(report["summary"]["total_sites"], 1);
    assert_eq!(report["summary"]["gone"], 1);
    assert_eq!(report["sites"][0]["kind"], "rule_binding");
    assert_eq!(report["sites"][0]["file_path"], "src/runtime.ts");
    assert_eq!(report["sites"][0]["state"], "gone");
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

    assert_eq!(report["summary"]["total_sites"], 1);
    assert_eq!(report["summary"]["touched"], 1);
    assert_eq!(report["sites"][0]["subject_id"], "rule_start");
    assert_eq!(report["sites"][0]["kind"], "rule_binding");
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

    assert_eq!(report["summary"]["total_sites"], 2);
    assert_eq!(report["summary"]["touched"], 2);
    assert!(report["sites"]
        .as_array()
        .unwrap()
        .iter()
        .all(|site| site["subject_id"] == "rule_start" && site["kind"] == "rule_binding"));
    assert_eq!(
        report["sites"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|site| site.get("line").is_some())
            .count(),
        1
    );
}
