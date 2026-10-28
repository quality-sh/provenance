//! A shard line that is not JSON is reported with its shard path and line.
use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

fn init(repo: &str) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["init", "--path", repo, "--scope", "default"])
        .assert()
        .success();
}

fn create_requirement(repo: &str, id: &str, statement: &str) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "requirements",
            "create",
            "--repo",
            repo,
            "--scope",
            "default",
            "--id",
            id,
            "--statement",
            statement,
            "--format",
            "json",
        ])
        .assert()
        .success();
}

// Implementation aid: the error text helps a person find the bad line; no Rule states it.
#[test]
fn a_blank_shard_line_is_named_by_shard_and_line() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_str().unwrap().to_string();
    init(&repo);
    create_requirement(
        &repo,
        "req_overtime",
        "Overtime must follow the award thresholds",
    );
    let path = dir
        .path()
        .join(".provenance/state/scopes/default/requirements/req.jsonl");
    let stored = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{stored}\n")).unwrap();

    for command in ["check", "health"] {
        Command::cargo_bin("provenance")
            .unwrap()
            .args([command, "--repo", repo.as_str()])
            .assert()
            .failure()
            .stderr(predicates::function::function(|text: &str| {
                text.replace('\\', "/")
                    .contains("requirements/req.jsonl line 2: EOF while parsing a value")
            }))
            .stderr(contains("line 1 column 0").not());
    }
}
