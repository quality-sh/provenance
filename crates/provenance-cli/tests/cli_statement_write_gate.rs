//! Direct Requirement and Rule writes apply the deterministic writing gate.

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use provenance_macros::verifies;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn initialized_repo() -> tempfile::TempDir {
    let repo = tempfile::tempdir().unwrap();
    provenance()
        .args([
            "init",
            "--path",
            repo.path().to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    repo
}

fn export(repo: &tempfile::TempDir) -> String {
    let output = provenance()
        .args([
            "export",
            "--repo",
            repo.path().to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
#[verifies("rule_ste_direct_statement_write_gate", examples)]
fn requirement_creation_rejects_a_deterministic_violation_without_writing_the_record() {
    let repo = initialized_repo();

    provenance()
        .args([
            "requirements",
            "create",
            "--repo",
            repo.path().to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            "req_rejected",
            "--statement",
            "Stop; then continue",
        ])
        .assert()
        .failure()
        .stderr(
            contains(r#""field":"statement""#)
                .and(contains(r#""standard":"ASD-STE100""#))
                .and(contains(r#""issue":9"#))
                .and(contains(r#""rule":"8.1""#))
                .and(contains(r#""start":4,"end":5"#)),
        );

    assert!(!export(&repo).contains("req_rejected"));
}

#[test]
#[verifies("rule_ste_direct_statement_write_gate", examples)]
fn rule_creation_rejects_a_deterministic_violation_without_writing_the_record() {
    let repo = initialized_repo();
    provenance()
        .args([
            "requirements",
            "create",
            "--repo",
            repo.path().to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            "req_anchor",
            "--statement",
            "The anchor requirement holds",
        ])
        .assert()
        .success();

    provenance()
        .args([
            "rules",
            "create",
            "--repo",
            repo.path().to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            "rule_rejected",
            "--requirement-id",
            "req_anchor",
            "--statement",
            "Use one sentence; do not join two",
        ])
        .assert()
        .failure()
        .stderr(
            contains(r#""field":"statement""#)
                .and(contains(r#""standard":"ASD-STE100""#))
                .and(contains(r#""issue":9"#))
                .and(contains(r#""rule":"8.1""#))
                .and(contains(r#""start":16,"end":17"#)),
        );

    assert!(!export(&repo).contains("rule_rejected"));
}
