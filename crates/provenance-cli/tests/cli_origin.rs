use assert_cmd::Command;
use serde_json::Value;

fn provenance_json(args: &[&str]) -> Value {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn provenance_json_stdin(args: &[&str], input: &str) -> Value {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args(args)
        .write_stdin(input)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

#[test]
#[allow(clippy::too_many_lines)]
fn cli_create_commands_preserve_origin_thread_and_message() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_string_lossy().to_string();

    Command::cargo_bin("provenance")
        .unwrap()
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
    provenance_json(&[
        "requirements",
        "create",
        "--repo",
        &repo,
        "--scope",
        "default",
        "--id",
        "req_origin_seed",
        "--statement",
        "Seed requirement for origin thread",
        "--format",
        "json",
    ]);
    let posted = provenance_json_stdin(
        &[
            "requirements",
            "req_origin_seed",
            "discussions",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--stdin",
            "--format",
            "json",
        ],
        r#"{"role":"user","body":"Promote this conversation into artifacts"}"#,
    );
    let discussion_id = posted["data"]["discussion_id"]
        .as_str()
        .unwrap()
        .to_string();
    let origin_thread = posted["data"]["thread_id"].as_str().unwrap().to_string();
    let origin_message = posted["data"]["root_message_id"]
        .as_str()
        .unwrap()
        .to_string();

    let source = provenance_json(&[
        "sources",
        "create",
        "--repo",
        &repo,
        "--scope",
        "default",
        "--id",
        "source_origin",
        "--name",
        "Origin Source",
        "--origin-thread",
        &origin_thread,
        "--origin-message",
        &origin_message,
        "--format",
        "json",
    ]);
    let requirement = provenance_json_stdin(
        &[
            "requirements",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--stdin",
            "--format",
            "json",
        ],
        &serde_json::json!({
            "id":"req_origin_child",
            "statement":"Origin child requirement",
            "origin":{
                "thread_id": origin_thread,
                "discussion_id": discussion_id,
                "message_id": origin_message
            }
        })
        .to_string(),
    );
    let resolution = provenance_json(&[
        "resolutions",
        "create",
        "--repo",
        &repo,
        "--scope",
        "default",
        "--id",
        "res_origin",
        "--title",
        "Origin resolution",
        "--requirement-id",
        "req_origin_child",
        "--position",
        "Use the promoted conversation",
        "--rationale",
        "The thread captured the decision",
        "--origin-thread",
        &origin_thread,
        "--origin-message",
        &origin_message,
        "--format",
        "json",
    ]);
    let rule = provenance_json(&[
        "rules",
        "create",
        "--repo",
        &repo,
        "--scope",
        "default",
        "--id",
        "rule_origin",
        "--requirement-id",
        "req_origin_child",
        "--resolution-id",
        "res_origin",
        "--statement",
        "Artifacts shall keep their origin conversation",
        "--origin-thread",
        &origin_thread,
        "--origin-message",
        &origin_message,
        "--format",
        "json",
    ]);

    for artifact in [&source, &requirement, &resolution, &rule] {
        assert_eq!(artifact["data"]["origin_thread"], origin_thread);
        assert_eq!(artifact["data"]["origin_message"], origin_message);
    }

    // The CLI-created Requirements enroll through the guarded journal, and a
    // review-bearing scope refuses a lossy export until export carries
    // journal history. Origin facts stay verifiable on the created records
    // above and through the journal reads.
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "export", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "review-bearing scopes require lossless import/export support",
        ));
}
