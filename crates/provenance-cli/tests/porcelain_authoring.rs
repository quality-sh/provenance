mod porcelain_authoring_support;

use predicates::prelude::PredicateBooleanExt as _;
use provenance_macros::verifies;
use porcelain_authoring_support::{initialized_repo, json_output, json_stdin_output, provenance};

#[test]
#[verifies("rule_porcelain_cli_target_action_order", examples)]
#[verifies("rule_porcelain_create_names_new_record", examples)]
fn target_first_create_uses_the_target_id_and_requires_an_explicit_type() {
    let (_directory, repo) = initialized_repo();

    let source = json_output(&[
        "source_target",
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
    assert_eq!(source["data"]["id"], "source_target");
    assert_eq!(source["data"]["source_type"], "policy");

    provenance()
        .args([
            "source_without_type",
            "create",
            "--repo",
            &repo,
            "--name",
            "Missing type",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("create requires --type"));
}

#[test]
#[verifies("rule_porcelain_parent_is_separate_input", examples)]
fn target_first_create_keeps_the_parent_separate_from_the_child_id() {
    let (_directory, repo) = initialized_repo();

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
#[verifies("rule_porcelain_relationship_patch_modes", examples)]
fn relationship_updates_preserve_unmodified_members_until_explicit_replacement() {
    let (_directory, repo) = initialized_repo();
    for (id, statement) in [
        ("req_first", "The first dependency exists."),
        ("req_second", "The second dependency exists."),
        ("req_replacement", "The replacement dependency exists."),
    ] {
        json_output(&[
            id,
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            statement,
            "--format",
            "json",
        ]);
    }

    let target = json_stdin_output(
        &[
            "req_target",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--stdin",
            "--format",
            "json",
        ],
        &serde_json::json!({
            "statement":"Relationship patches preserve members that they do not name.",
            "depends_on":["req_first"]
        }),
    );
    assert_eq!(target["data"]["depends_on"], serde_json::json!(["req_first"]));

    let added = json_stdin_output(
        &[
            "req_target",
            "update",
            "--repo",
            &repo,
            "--if-match",
            target["data"]["edit"]["etag"].as_str().unwrap(),
            "--stdin",
            "--format",
            "json",
        ],
        &serde_json::json!({"relationships":{"depends_on":{"add":["req_second"]}}}),
    );
    assert_eq!(
        added["data"]["depends_on"],
        serde_json::json!(["req_first", "req_second"])
    );

    let removed = json_stdin_output(
        &[
            "req_target",
            "update",
            "--repo",
            &repo,
            "--if-match",
            added["data"]["edit"]["etag"].as_str().unwrap(),
            "--stdin",
            "--format",
            "json",
        ],
        &serde_json::json!({"relationships":{"depends_on":{"remove":["req_first"]}}}),
    );
    assert_eq!(removed["data"]["depends_on"], serde_json::json!(["req_second"]));

    let replacement = json_stdin_output(
        &[
            "req_target",
            "update",
            "--repo",
            &repo,
            "--if-match",
            removed["data"]["edit"]["etag"].as_str().unwrap(),
            "--stdin",
            "--format",
            "json",
        ],
        &serde_json::json!({"relationships":{"depends_on":["req_replacement"]}}),
    );
    assert_eq!(
        replacement["data"]["depends_on"],
        serde_json::json!(["req_replacement"])
    );
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

fn named_action_repo() -> (tempfile::TempDir, String) {
    let (directory, repo) = initialized_repo();
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

    (directory, repo)
}

#[test]
#[verifies("rule_porcelain_existing_action_infers_kind", examples)]
#[verifies("rule_porcelain_named_domain_actions", examples)]
fn target_first_topic_actions_infer_kind_and_update_claim_state() {
    let (_directory, repo) = named_action_repo();
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
}

#[test]
#[verifies("rule_porcelain_existing_action_infers_kind", examples)]
#[verifies("rule_porcelain_named_domain_actions", examples)]
fn target_first_question_actions_infer_kind_and_reject_a_mismatched_action() {
    let (_directory, repo) = named_action_repo();
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
        .stderr(predicates::str::contains("unsupported action options"))
        .stderr(predicates::str::contains("unrecognized subcommand").not());
}

#[test]
#[verifies("rule_porcelain_existing_action_infers_kind", examples)]
#[verifies("rule_porcelain_named_domain_actions", examples)]
fn target_first_requirement_submit_infers_kind_and_records_the_submission() {
    let (_directory, repo) = named_action_repo();
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
        &serde_json::json!({
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
}

#[test]
#[verifies("rule_porcelain_update_keeps_preconditions", examples)]
fn target_first_mutations_keep_parent_version_and_existence_preconditions() {
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
