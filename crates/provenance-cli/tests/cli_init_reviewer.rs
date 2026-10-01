use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use std::path::Path;

fn manifest(repo: &Path) -> Value {
    serde_json::from_slice(
        &std::fs::read(repo.join(".provenance/state/manifest.json")).unwrap(),
    )
    .unwrap()
}

fn init(repo: &Path) -> Command {
    let mut command = Command::cargo_bin("provenance").unwrap();
    command.args(["init", "--path", repo.to_str().unwrap()]);
    command
}

fn without_git_identity(command: &mut Command, temporary: &Path) {
    let config = temporary.join("empty-gitconfig");
    std::fs::write(&config, "").unwrap();
    command
        .env("GIT_CONFIG_GLOBAL", config)
        .env("GIT_CONFIG_NOSYSTEM", "1");
}

#[test]
fn init_uses_the_normalized_git_email_as_the_reviewer() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&repo)
        .status()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "Jane.Doe+Review@example.com"])
        .current_dir(&repo)
        .status()
        .unwrap();

    init(&repo)
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Reviewer set to \"jane_doe_review_example_com\". Change reviewers with `provenance init --path",
        ));

    assert_eq!(
        manifest(&repo)["disposition_actor_ids"],
        serde_json::json!(["jane_doe_review_example_com"])
    );
}

#[test]
fn init_keeps_the_explicit_reviewer() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");

    init(&repo)
        .args(["--disposition-actor-id", "maintainer"])
        .assert()
        .success();

    assert_eq!(
        manifest(&repo)["disposition_actor_ids"],
        serde_json::json!(["maintainer"])
    );
}

#[test]
fn non_interactive_init_without_a_git_identity_warns_about_read_only_review() {
    let temporary = tempfile::tempdir().unwrap();
    let repo = temporary.path().join("repo");
    let mut command = init(&repo);
    without_git_identity(&mut command, temporary.path());

    command.assert().success().stderr(predicate::str::contains(
        format!(
            "Warning: No reviewer is configured. Review will be read-only. Add a reviewer with `provenance init --path {} --disposition-actor-id <reviewer-id>`.",
            repo.display()
        ),
    ));

    assert_eq!(
        manifest(&repo)["disposition_actor_ids"],
        serde_json::json!([])
    );
}
