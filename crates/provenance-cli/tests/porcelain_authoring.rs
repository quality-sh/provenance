#[path = "porcelain_authoring/named_actions.rs"]
mod named_actions;
mod porcelain_authoring_support;

use porcelain_authoring_support::{initialized_repo, json_output, json_stdin_output, provenance};
use predicates::prelude::PredicateBooleanExt as _;
use provenance_macros::verifies;

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
    assert!(parent["data"]["decision"]["pending"]["proposal_id"].is_string());
    assert_eq!(
        parent["data"]["decision"]["pending"]["revision"],
        parent["data"]["edit"]["revision"]
    );

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
    assert!(child["data"]["decision"]["pending"]["proposal_id"].is_string());
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
    assert_eq!(
        target["data"]["depends_on"],
        serde_json::json!(["req_first"])
    );

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
    assert_eq!(
        removed["data"]["depends_on"],
        serde_json::json!(["req_second"])
    );

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
        "--url-json",
        "null",
        "--format",
        "json",
    ]);
    assert!(cleared["data"]["url"].is_null());
    assert_eq!(cleared["data"]["reference"], "clause 1");

    let literal = json_output(&[
        "source_patch",
        "update",
        "--repo",
        &repo,
        "--url",
        "null",
        "--format",
        "json",
    ]);
    assert_eq!(literal["data"]["url"], "null");
    assert_eq!(literal["data"]["reference"], "clause 1");
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
        .stderr(predicates::str::contains("requirement_edit_conflict"));
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
        .stdout(predicates::str::contains("<TARGET> [ACTION]"))
        .stdout(predicates::str::contains("--type"));

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
