use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::{json, Value};

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
#[verifies("rule_porcelain_cli_target_action_order", examples)]
#[verifies("rule_porcelain_create_names_new_record", examples)]
fn collection_named_targets_accept_explicit_create_update_and_get() {
    let (_directory, repo) = initialized_repo();

    let created = json_output(&[
        "--repo",
        &repo,
        "sources",
        "create",
        "--type",
        "source",
        "--name",
        "Collection ID",
        "--format",
        "json",
    ]);
    assert_eq!(created["data"]["id"], "sources");

    let updated = json_output(&[
        "sources",
        "update",
        "--repo",
        &repo,
        "--name",
        "Changed",
        "--format",
        "json",
    ]);
    assert_eq!(updated["data"]["id"], "sources");
    assert_eq!(updated["data"]["name"], "Changed");

    let read = json_output(&["sources", "get", "--repo", &repo, "--format", "json"]);
    assert_eq!(read["record"]["id"], "sources");
}

#[test]
#[verifies("rule_porcelain_cli_target_action_order", examples)]
#[verifies("rule_porcelain_named_domain_actions", examples)]
fn explicit_named_actions_win_for_collection_named_targets() {
    let (_directory, repo) = initialized_repo();
    json_output(&[
        "requirements",
        "create",
        "--repo",
        &repo,
        "--id",
        "req_routing",
        "--statement",
        "The grammar routes explicit target actions.",
        "--format",
        "json",
    ]);
    json_output(&[
        "topics",
        "create",
        "--repo",
        &repo,
        "--id",
        "topics",
        "--requirement-id",
        "req_routing",
        "--title",
        "Routing",
        "--format",
        "json",
    ]);
    json_output(&[
        "questions",
        "create",
        "--repo",
        &repo,
        "--id",
        "questions",
        "--topic-id",
        "topics",
        "--question",
        "Does the grammar select the action?",
        "--method",
        "research",
        "--format",
        "json",
    ]);

    let claimed = json_output(&[
        "topics",
        "claim",
        "--repo",
        &repo,
        "--actor",
        "worker",
        "--format",
        "json",
    ]);
    assert_eq!(claimed["data"]["claimed_by"], "worker");
    let answered = json_output(&[
        "questions",
        "answer",
        "--repo",
        &repo,
        "--answer",
        "Yes.",
        "--format",
        "json",
    ]);
    assert_eq!(answered["data"]["status"], "answered");
}

#[test]
fn reserved_commands_global_flags_and_legacy_catalog_forms_keep_their_meaning() {
    let (_directory, repo) = initialized_repo();
    provenance().arg("--help").assert().success();
    provenance()
        .args(["check", "--repo", &repo])
        .assert()
        .success();

    let legacy = json_output(&[
        "sources",
        "create",
        "--repo",
        &repo,
        "--id",
        "source_legacy",
        "--name",
        "Legacy",
        "--format",
        "json",
    ]);
    assert_eq!(legacy["data"]["id"], "source_legacy");
    let listed = json_output(&[
        "--repo",
        &repo,
        "sources",
        "list",
        "--format",
        "json",
    ]);
    assert!(listed["data"].as_array().unwrap().iter().any(|source| {
        source["id"] == json!("source_legacy")
    }));
}
