use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt as _;
use serde_json::Value;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}
fn initialized_repo() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
        .args([
            "init",
            "--path",
            &repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    (directory, repo)
}

fn json_output(arguments: &[&str]) -> Value {
    let output = provenance().args(arguments).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn target_first_create_uses_the_target_type_and_separate_parent_fields() {
    let (_directory, repo) = initialized_repo();

    let source = json_output(&[
        "source_parent",
        "create",
        "--type",
        "source",
        "--repo",
        &repo,
        "--name",
        "Parent source",
        "--source-type",
        "policy",
        "--format",
        "json",
    ]);
    assert_eq!(source["data"]["id"], "source_parent");
    assert_eq!(source["data"]["source_type"], "policy");

    let parent = json_output(&[
        "req_parent",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The parent requirement exists.",
        "--format",
        "json",
    ]);
    assert_eq!(parent["data"]["id"], "req_parent");

    let child = json_output(&[
        "req_child",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The child requirement refines its parent.",
        "--refines",
        "req_parent",
        "--format",
        "json",
    ]);
    assert_eq!(child["data"]["id"], "req_child");
    assert_eq!(child["data"]["refines"], "req_parent");
}

#[test]
fn target_first_update_preserves_omissions_and_honors_null_clear() {
    let (_directory, repo) = initialized_repo();
    json_output(&[
        "source_patch",
        "create",
        "--type",
        "source",
        "--repo",
        &repo,
        "--name",
        "Original",
        "--source-type",
        "policy",
        "--url",
        "https://example.test/policy",
        "--reference",
        "clause 1",
        "--format",
        "json",
    ]);

    let edited = json_output(&[
        "source_patch",
        "update",
        "--repo",
        &repo,
        "--name",
        "Edited",
        "--format",
        "json",
    ]);
    assert_eq!(edited["data"]["name"], "Edited");
    assert_eq!(edited["data"]["url"], "https://example.test/policy");
    assert_eq!(edited["data"]["reference"], "clause 1");

    let cleared = json_output(&[
        "source_patch",
        "update",
        "--repo",
        &repo,
        "--url",
        "null",
        "--format",
        "json",
    ]);
    assert!(cleared["data"]["url"].is_null());
    assert_eq!(cleared["data"]["reference"], "clause 1");
}

#[test]
fn target_first_named_actions_keep_the_existing_domain_preconditions() {
    let (_directory, repo) = initialized_repo();
    json_output(&[
        "req_actions",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The named actions keep their preconditions.",
        "--format",
        "json",
    ]);
    json_output(&[
        "topic_actions",
        "create",
        "--type",
        "topic",
        "--repo",
        &repo,
        "--requirement-id",
        "req_actions",
        "--title",
        "Named actions",
        "--format",
        "json",
    ]);
    json_output(&[
        "question_actions",
        "create",
        "--type",
        "question",
        "--repo",
        &repo,
        "--topic-id",
        "topic_actions",
        "--question",
        "Does the action resolve its target?",
        "--method",
        "research",
        "--format",
        "json",
    ]);

    let claimed = json_output(&[
        "topic_actions",
        "claim",
        "--repo",
        &repo,
        "--actor",
        "worker",
        "--format",
        "json",
    ]);
    assert_eq!(claimed["data"]["claimed_by"], "worker");
    let released = json_output(&[
        "topic_actions",
        "release",
        "--repo",
        &repo,
        "--format",
        "json",
    ]);
    assert!(released["data"]["claimed_by"].is_null());

    let answered = json_output(&[
        "question_actions",
        "answer",
        "--repo",
        &repo,
        "--answer",
        "Yes.",
        "--format",
        "json",
    ]);
    assert_eq!(answered["data"]["status"], "answered");
    provenance()
        .args([
            "question_actions",
            "claim",
            "--repo",
            &repo,
            "--actor",
            "worker",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("invalid_update"))
        .stderr(predicates::str::contains("unrecognized subcommand").not());
}

#[test]
fn target_first_mutations_refuse_bad_identity_and_stale_versions() {
    let (_directory, repo) = initialized_repo();
    provenance()
        .args([
            "req_bad_parent",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            "This parent does not exist.",
            "--refines",
            "req_missing",
        ])
        .assert()
        .failure();

    json_output(&[
        "req_stale",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "Stale changes are refused.",
        "--format",
        "json",
    ]);
    provenance()
        .args([
            "req_stale",
            "update",
            "--repo",
            &repo,
            "--if-match",
            "stale-etag",
            "--description",
            "This write is stale.",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("invalid_update"));
    provenance()
        .args([
            "missing_target",
            "update",
            "--repo",
            &repo,
            "--name",
            "No record",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("record does not exist"));
}
