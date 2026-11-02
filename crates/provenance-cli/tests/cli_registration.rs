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

#[test]
fn one_argument_can_contain_spaces_and_punctuation() {
    let (_directory, repo) = init();
    let output = provenance()
        .args([
            "source_draft_policy",
            "create",
            "--type",
            "source",
            "--repo",
            &repo,
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
