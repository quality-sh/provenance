use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt as _;
use provenance_macros::verifies;
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

fn json_stdin_output(arguments: &[&str], input: Value) -> Value {
    let output = provenance()
        .args(arguments)
        .write_stdin(serde_json::to_vec(&input).unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
#[verifies("rule_porcelain_cli_target_action_order", examples)]
#[verifies("rule_porcelain_create_names_new_record", examples)]
#[verifies("rule_porcelain_parent_is_separate_input", examples)]
#[verifies("rule_porcelain_relationship_patch_modes", examples)]
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

    json_output(&[
        "req_dependency",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The dependency requirement exists.",
        "--format",
        "json",
    ]);

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

    let delta = json_stdin_output(
        &[
            "req_child",
            "update",
            "--repo",
            &repo,
            "--if-match",
            child["data"]["edit"]["etag"].as_str().unwrap(),
            "--stdin",
            "--format",
            "json",
        ],
        serde_json::json!({"relationships":{"depends_on":{"add":["req_dependency"]}}}),
    );
    assert_eq!(delta["data"]["depends_on"], serde_json::json!(["req_dependency"]));

    let replacement = json_stdin_output(
        &[
            "req_child",
            "update",
            "--repo",
            &repo,
            "--if-match",
            delta["data"]["edit"]["etag"].as_str().unwrap(),
            "--stdin",
            "--format",
            "json",
        ],
        serde_json::json!({"relationships":{"depends_on":["req_parent"]}}),
    );
    assert_eq!(replacement["data"]["depends_on"], serde_json::json!(["req_parent"]));
}

#[test]
#[verifies("rule_porcelain_cli_readable_json", examples)]
#[verifies("rule_porcelain_update_preserves_omissions", examples)]
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

    let readable = provenance()
        .args([
            "source_patch",
            "update",
            "--repo",
            &repo,
            "--name",
            "Edited",
        ])
        .output()
        .unwrap();
    assert!(readable.status.success());
    let readable = String::from_utf8(readable.stdout).unwrap();
    assert!(readable.contains("update source source_patch"));
    assert!(readable.contains(r#""name": "Edited""#));
    assert!(readable.contains("https://example.test/policy"));
    assert!(readable.contains("clause 1"));

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
#[verifies("rule_porcelain_existing_action_infers_kind", examples)]
#[verifies("rule_porcelain_named_domain_actions", examples)]
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

    let submitted = json_stdin_output(
        &[
            "req_actions",
            "submit",
            "--repo",
            &repo,
            "--stdin",
            "--format",
            "json",
        ],
        serde_json::json!({
            "actor":"agent",
            "proposal_id":"proposal_cli_target",
            "proposal_key":"cli-target",
            "title":"CLI target",
            "summary":"The target-first action submits this Requirement.",
            "source_ids":[],
            "evidence_references":[],
            "builds_on":[]
        }),
    );
    assert_eq!(submitted["data"]["requirement_id"], "req_actions");
    assert_eq!(submitted["data"]["fact"], "submitted");

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
        .stderr(predicates::str::contains("unsupported action options"))
        .stderr(predicates::str::contains("unrecognized subcommand").not());
}

#[test]
#[verifies("rule_porcelain_update_keeps_preconditions", examples)]
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

#[test]
fn target_actions_keep_help_global_context_and_flag_like_values() {
    provenance()
        .args(["new_source", "create", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("provenance <new-id> create"));

    let (_directory, repo) = initialized_repo();
    let source = json_output(&[
        "--repo",
        &repo,
        "source_flag_value",
        "create",
        "--type",
        "source",
        "--name",
        "--repo",
        "--format",
        "json",
    ]);
    assert_eq!(source["data"]["name"], "--repo");
}
