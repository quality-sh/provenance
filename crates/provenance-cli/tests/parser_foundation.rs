use assert_cmd::Command;
use predicates::str::contains;
use serde_json::{json, Value};

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn repo() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().to_string_lossy().into_owned();
    provenance()
        .args(["init", "--path", &path, "--scope", "default", "--path-prefix", "."])
        .assert()
        .success();
    (directory, path)
}

#[test]
fn catalog_refuses_a_repeated_scalar_before_a_write() {
    let (_directory, path) = repo();
    provenance()
        .args([
            "sources", "create", "--repo", &path, "--id", "source_duplicate",
            "--name", "First", "--name", "Second",
        ])
        .assert()
        .code(2)
        .stderr(contains("--name"));
    let listed = provenance()
        .args(["sources", "list", "--repo", &path])
        .output()
        .unwrap();
    assert!(listed.status.success());
    let page: Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(page["data"]["items"], json!([]));
}

#[test]
fn shared_flags_and_equals_values_work_at_all_target_positions() {
    let (_directory, path) = repo();
    provenance()
        .args([
            "--repo", &path, "source_positions", "create", "--type=source",
            "--name=--repo", "--format=json",
        ])
        .assert()
        .success();
    let read = provenance()
        .args(["source_positions", "--scope=default", "get", "--repo", &path, "--format=json"])
        .output()
        .unwrap();
    assert!(read.status.success(), "{}", String::from_utf8_lossy(&read.stderr));
    let value: Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(value["record"]["name"], "--repo");
}

#[test]
fn search_accepts_equals_and_refuses_duplicate_scalars() {
    let (_directory, path) = repo();
    provenance()
        .args(["search", "--repo", &path, "--text=needle", "--kind=source", "--format=json"])
        .assert()
        .success();
    provenance()
        .args(["search", "--repo", &path, "--text=one", "--text=two"])
        .assert()
        .code(2)
        .stderr(contains("--text"));
}

#[test]
fn body_flag_and_stdin_conflict_before_a_write() {
    let (_directory, path) = repo();
    provenance()
        .args(["sources", "create", "--repo", &path, "--name", "Flag", "--stdin"])
        .write_stdin(r#"{"id":"source_stdin","name":"Body"}"#)
        .assert()
        .code(2)
        .stderr(contains("--stdin"));
}

#[test]
fn help_and_unknown_options_have_clap_exit_codes_without_a_repository() {
    let directory = tempfile::tempdir().unwrap();
    provenance()
        .current_dir(directory.path())
        .args(["search", "--help"])
        .assert()
        .code(0)
        .stdout(contains("--cursor"));
    provenance()
        .current_dir(directory.path())
        .args(["search", "--unknown=value"])
        .assert()
        .code(2)
        .stderr(contains("--unknown"));
}

#[test]
fn command_keywords_are_refused_as_record_ids_on_both_cli_write_forms() {
    let (_directory, path) = repo();
    provenance()
        .args(["sources", "create", "--repo", &path, "--id", "search", "--name", "Search"])
        .assert()
        .failure()
        .stderr(contains("reserved record ID search"));
    provenance()
        .args(["sources", "create", "--repo", &path, "--stdin"])
        .write_stdin(r#"{"id":"check","name":"Check"}"#)
        .assert()
        .failure()
        .stderr(contains("reserved record ID check"));
}
