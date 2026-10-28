//! Binding checks and scope reports resolve the selected scope against the
//! manifest before they read code or state. A scope sees only the files it
//! owns: those under its path prefix and under no longer prefix; an undeclared scope, a plain directory, and a repository
//! subdirectory refuse instead of reporting an empty success.

use assert_cmd::Command;
use predicates::prelude::*;
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

mod support {
    pub mod review_paths;
}
use support::review_paths::{init_scope as init, provenance};

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

/// One Rule for each scope, implemented and verified in `<dir>/lib.rs`.
fn bound_scopes(repo: &Path, layout: &[(&str, &str)]) {
    for (scope, prefix) in layout {
        init(repo, scope, prefix);
    }
    for (scope, prefix) in layout {
        active_rule(repo, scope, &format!("rule_{scope}"));
        let dir = if *prefix == "." { "src" } else { prefix };
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
}

/// Each scope passes a strict scan with one owned file, and the repository
/// binding check reports no unknown Rule.
fn assert_each_scope_sees_only_its_own_bindings(repo: &Path, scopes: &[&str]) {
    for scope in scopes {
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
#[verifies("rule_binding_scope_longest_prefix", examples)]
fn binding_checks_validate_each_scope_against_the_files_under_its_prefix() {
    let directory = tempfile::tempdir().unwrap();
    bound_scopes(directory.path(), &[("alpha", "a"), ("beta", "b")]);

    assert_each_scope_sees_only_its_own_bindings(directory.path(), &["alpha", "beta"]);
}

#[test]
#[verifies("rule_binding_scope_longest_prefix", examples)]
fn a_root_scope_does_not_see_the_bindings_of_a_scope_under_it() {
    let directory = tempfile::tempdir().unwrap();
    bound_scopes(directory.path(), &[("root", "."), ("app", "app")]);

    assert_each_scope_sees_only_its_own_bindings(directory.path(), &["root", "app"]);
}

#[test]
#[verifies("rule_binding_scope_longest_prefix", examples)]
fn a_nested_scope_owns_its_files_and_the_outer_scope_does_not() {
    let directory = tempfile::tempdir().unwrap();
    bound_scopes(directory.path(), &[("outer", "a"), ("inner", "a/b")]);

    assert_each_scope_sees_only_its_own_bindings(directory.path(), &["outer", "inner"]);
}

// Implementation aid: a scan path outside the repository keeps its files in a `.` scope.
#[test]
fn a_root_scope_keeps_scanned_files_outside_the_repository() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    init(repo, "root", ".");
    init(repo, "app", "app");
    std::fs::create_dir_all(repo.join("app")).unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(
        outside.path().join("lib.rs"),
        "#[rule(\"rule_root\")]\nfn implemented() {}\n",
    )
    .unwrap();

    for (scope, files) in [("root", 1), ("app", 0)] {
        let output = provenance()
            .args([
                "coverage",
                "scan",
                "--repo",
                repo.to_str().unwrap(),
                "--path",
                outside.path().to_str().unwrap(),
                "--scope",
                scope,
                "--format",
                "json",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{scope}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["files_scanned"], files, "{scope}: {report}");
    }
}

#[test]
#[verifies("rule_coverage_names_missing_scope_directory", examples)]
fn a_scope_whose_directory_is_absent_gets_a_warning_that_names_it() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    init(repo, "app", "apps/web");

    let output = strict_scan(repo, "app").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let warnings = report["warnings"].as_array().unwrap();
    assert!(
        warnings.iter().any(|warning| warning["message"]
            == "scope `app` has path prefix `apps/web`, which names no directory in the repository"
            && warning["file_path"] == "apps/web"),
        "{report}"
    );
}

#[test]
#[verifies("rule_graph_report_refuses_undeclared_scope", examples)]
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
#[verifies("rule_graph_report_refuses_undeclared_scope", examples)]
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

// Implementation aid: a scan in a directory with no graph and no enclosing
// repository refuses and creates no `.provenance` directory.
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
#[verifies("rule_command_names_enclosing_repository", examples)]
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
