use assert_cmd::Command;
use serde_json::Value;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn init() -> (tempfile::TempDir, String) {
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

fn create_source(repo: &str, id: &str) {
    provenance()
        .args([
            "sources", "create", "--repo", repo, "--id", id, "--name", id,
        ])
        .assert()
        .success();
}

#[test]
fn flag_values_are_consumed_once_even_when_they_look_like_help() {
    let (_directory, repo) = init();
    let output = provenance()
        .args([
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "source_help_name",
            "--name",
            "--help",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout.first().copied(), Some(b'{'));
    let output: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output["data"]["name"], "--help");
}

#[test]
fn one_argument_can_contain_spaces_and_punctuation() {
    let (_directory, repo) = init();
    let output = provenance()
        .args([
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "source_draft_policy",
            "--name",
            "[Draft] policy",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output["data"]["name"], "[Draft] policy");
}

#[test]
fn discussion_member_address_selects_the_read_route() {
    let (_directory, repo) = init();
    create_source(&repo, "source_discussion_address");
    let started = provenance()
        .args([
            "sources",
            "source_discussion_address",
            "discussions",
            "create",
            "--repo",
            &repo,
            "--role",
            "user",
            "--body",
            "Read this discussion.",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        started.status.success(),
        "{}",
        String::from_utf8_lossy(&started.stderr)
    );
    let started: Value = serde_json::from_slice(&started.stdout).unwrap();
    let discussion_id = started["data"]["discussion_id"].as_str().unwrap();
    for messages in [false, true] {
        let mut args = vec![
            "sources",
            "source_discussion_address",
            "discussions",
            discussion_id,
        ];
        if messages {
            args.push("messages");
        }
        args.push("get");
        args.extend(["--repo", &repo, "--format", "json"]);
        let output = provenance().args(args).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output: Value = serde_json::from_slice(&output.stdout).unwrap();
        if messages {
            assert_eq!(output["data"]["items"].as_array().unwrap().len(), 1);
        } else {
            assert_eq!(output["data"]["discussion"]["discussion_id"], discussion_id);
        }
    }
}
