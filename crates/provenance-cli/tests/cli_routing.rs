use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt as _;

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

#[test]
fn catalog_flag_like_values_are_not_taken_as_global_options() {
    let (_directory, repo) = initialized_repo();

    provenance()
        .args([
            "sources",
            "create",
            "--id",
            "source_flag_like_value",
            "--name",
            "--repo",
            "--repo",
            &repo,
        ])
        .assert()
        .success();

    let output = provenance()
        .args([
            "source_flag_like_value",
            "get",
            "--repo",
            &repo,
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
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["record"]["value"]["name"], "--repo");
}

#[test]
fn help_keeps_catalog_grammar_with_trailing_global_options() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();

    provenance()
        .args(["sources", "--help", "--repo", &repo])
        .assert()
        .success()
        .stdout(predicates::str::contains("Catalog commands for sources"));
}

#[test]
fn explicit_get_reads_a_record_whose_id_is_get() {
    let (_directory, repo) = initialized_repo();
    provenance()
        .args([
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "get",
            "--name",
            "Get target",
        ])
        .assert()
        .success();

    let output = provenance()
        .args(["get", "get", "--repo", &repo, "--format", "json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["record"]["id"], "get");
}

#[test]
fn invalid_explicit_get_options_do_not_fall_back_to_the_catalog() {
    provenance()
        .args(["sources", "get", "--unknown-option", "value"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unsupported read options"))
        .stderr(predicates::str::contains("catalog does not declare").not());
}
