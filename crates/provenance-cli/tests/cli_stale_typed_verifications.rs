use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use serde_json::{json, Value};
use std::path::Path;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn git(repo: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
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
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", message]);
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
    let input = json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "runtime",
        "declared_by": "spec://typescript/runtime",
        "requirements": [{
            "key": "workflows",
            "statement": "Accepted workflows execute"
        }],
        "rules": [
            {
                "key": "start",
                "id": "rule_start",
                "requirement": "workflows",
                "statement": "Accepted workflows start"
            },
            {
                "key": "resume",
                "id": "rule_resume",
                "requirement": "workflows",
                "statement": "Suspended workflows resume"
            }
        ]
    });
    provenance()
        .args([
            "sdk",
            "apply",
            "--repo",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .write_stdin(serde_json::to_vec(&input).unwrap())
        .assert()
        .success();
    directory
}

fn verify(repo: &Path, rule: &str, key: &str, file: &str) {
    provenance()
        .args([
            "sdk",
            "begin-verification",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .write_stdin(
            serde_json::to_vec(&json!({
                "rule": rule,
                "key": key,
                "method": "examples",
                "declared_by": "ci://typescript",
                "file": file
            }))
            .unwrap(),
        )
        .assert()
        .success();
}

fn verification_sites(repo: &Path, base: &str, head: &str) -> Vec<Value> {
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
    serde_json::from_slice::<Value>(&output).unwrap()["sites"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|site| site["kind"] == "verification")
        .cloned()
        .collect()
}

#[test]
fn a_deleted_test_file_leaves_its_verification_binding_gone() {
    let directory = init_repo();
    write(
        directory.path(),
        "tests/expiry.test.ts",
        "// exercises start\n",
    );
    let base = commit(directory.path(), "add the test");
    verify(
        directory.path(),
        "rule_start",
        "expiry",
        "tests/expiry.test.ts",
    );
    std::fs::remove_file(directory.path().join("tests/expiry.test.ts")).unwrap();
    let head = commit(directory.path(), "remove the test");

    let sites = verification_sites(directory.path(), &base, &head);

    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0]["subject_id"], "rule_start");
    assert_eq!(sites[0]["state"], "gone");
}

#[test]
fn a_retired_verification_binding_is_not_a_stale_site() {
    let directory = init_repo();
    write(
        directory.path(),
        "tests/expiry.test.ts",
        "// exercises start\n",
    );
    let base = commit(directory.path(), "add the test");
    verify(
        directory.path(),
        "rule_start",
        "expiry",
        "tests/expiry.test.ts",
    );
    verify(
        directory.path(),
        "rule_resume",
        "expiry",
        "tests/expiry.test.ts",
    );
    std::fs::remove_file(directory.path().join("tests/expiry.test.ts")).unwrap();
    let head = commit(directory.path(), "remove the test");

    let sites = verification_sites(directory.path(), &base, &head);

    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0]["subject_id"], "rule_resume");
}
