//! Binding checks and scope reports resolve the selected scope against the
//! manifest before they read code or state. A scope sees only the files under
//! its path prefix; an undeclared scope, a plain directory, and a repository
//! subdirectory refuse instead of reporting an empty success.

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};
use std::path::Path;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn init(repo: &Path, scope: &str, prefix: &str) {
    let mut args = vec![
        "init",
        "--path",
        repo.to_str().unwrap(),
        "--scope",
        scope,
        "--path-prefix",
        prefix,
    ];
    if !repo.join(".provenance").exists() {
        args.extend(["--disposition-actor-id", "reviewer"]);
    }
    provenance().args(args).assert().success();
}

fn active_rule(repo: &Path, scope: &str, rule: &str) {
    let repo = repo.to_str().unwrap();
    let requirement = format!("req_{scope}");
    provenance()
        .args([
            "requirements",
            "create",
            "--repo",
            repo,
            "--scope",
            scope,
            "--id",
            &requirement,
            "--statement",
            "The system records topics.",
        ])
        .assert()
        .success();
    provenance()
        .args([
            "rules", "create", "--repo", repo, "--scope", scope, "--stdin",
        ])
        .write_stdin(
            json!({
                "id": rule,
                "requirement_ids": [requirement],
                "resolution_ids": [],
                "statement": "Payroll follows the current policy",
                "status": "active",
                "severity": "high"
            })
            .to_string(),
        )
        .assert()
        .success();
}

fn strict_scan(repo: &Path, scope: &str) -> Command {
    let mut command = provenance();
    command.args([
        "coverage",
        "scan",
        "--repo",
        repo.to_str().unwrap(),
        "--path",
        repo.to_str().unwrap(),
        "--scope",
        scope,
        "--validate-rules",
        "--strict",
        "--format",
        "json",
    ]);
    command
}

#[test]
fn binding_checks_validate_each_scope_against_the_files_under_its_prefix() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    init(repo, "alpha", "a");
    init(repo, "beta", "b");
    for (scope, dir) in [("alpha", "a"), ("beta", "b")] {
        active_rule(repo, scope, &format!("rule_{scope}"));
        std::fs::create_dir_all(repo.join(dir)).unwrap();
        std::fs::write(
            repo.join(dir).join("lib.rs"),
            format!(
                "#[rule(\"rule_{scope}\")]\nfn implemented() {{}}\n\
                 #[verifies(\"rule_{scope}\", examples)]\nfn verified() {{}}\n"
            ),
        )
        .unwrap();
    }

    for scope in ["alpha", "beta"] {
        let output = strict_scan(repo, scope).output().unwrap();
        assert!(
            output.status.success(),
            "{scope}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["warnings"], json!([]), "{scope}");
        assert_eq!(report["files_scanned"], 1, "{scope}");
    }
    provenance()
        .args(["check", "--bindings", "--repo", repo.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("unknown rule id").not());
}

#[test]
fn coverage_scan_refuses_a_scope_the_manifest_does_not_declare() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    init(repo, "app", ".");

    for scope in ["default", "defualt"] {
        strict_scan(repo, scope)
            .assert()
            .failure()
            .stdout(predicate::str::is_empty())
            .stderr(predicate::str::contains(format!(
                "unknown scope `{scope}`; declared scopes: app"
            )));
    }
}

#[test]
fn scope_reports_refuse_a_scope_the_manifest_does_not_declare() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    init(repo, "app", ".");

    for command in ["gaps", "health", "orphans", "export"] {
        provenance()
            .args([command, "--repo", repo.to_str().unwrap(), "--scope", "nope"])
            .assert()
            .failure()
            .stdout(predicate::str::is_empty())
            .stderr(predicate::str::contains(
                "unknown scope `nope`; declared scopes: app",
            ));
    }
}

#[test]
fn coverage_scan_outside_a_repository_refuses_without_writing() {
    let directory = tempfile::tempdir().unwrap();
    let plain = directory.path();
    std::fs::write(plain.join("lib.rs"), "fn f() {}\n").unwrap();

    strict_scan(plain, "default")
        .assert()
        .failure()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("not initialized"));

    assert!(!plain.join(".provenance").exists());
}

#[test]
fn commands_in_a_subdirectory_name_the_enclosing_repository() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    init(repo, "default", ".");
    let sub = repo.join("sub/deeper");
    std::fs::create_dir_all(&sub).unwrap();
    let root = std::fs::canonicalize(repo).unwrap();
    let hint = format!("run the command with --repo {}", root.display());

    for args in [
        vec!["search", "--text", "topic"],
        vec!["req_a"],
        vec!["check", "--quiet"],
        vec![
            "coverage",
            "scan",
            "--path",
            ".",
            "--scope",
            "default",
            "--validate-rules",
        ],
    ] {
        provenance()
            .current_dir(&sub)
            .args(&args)
            .assert()
            .failure()
            .stderr(predicate::str::contains(hint.as_str()))
            .stderr(predicate::str::contains("run `provenance init`").not());
    }
    assert!(!sub.join(".provenance").exists());
}
