//! Implementation aid with no Rule binding: a stored record has one format and
//! carries no format field. The tests read hand-written rows through
//! `provenance <id> get`, so they check the stored format and not one writer.
use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

const PLAIN: &str = r#"{"scope_id":"default","id":"req_plain","statement":"Records have one stored format.","status":"active"}"#;
const EXTENDED: &str = r#"{"scope_id":"default","id":"req_extended","statement":"Records keep only known fields.","status":"active","extension":{"owner":"newer-tool"}}"#;

#[test]
fn a_record_without_a_format_field_reads() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_str().unwrap();
    init_with_requirements(dir.path(), &[PLAIN]);

    get(repo, "req_plain")
        .success()
        .stdout(contains("Records have one stored format."))
        .stdout(contains("schema_version").not());
}

/// An open family reader ignores a field that its record type does not hold:
/// the record reads and the read output does not show the field.
#[test]
fn a_record_with_an_unknown_field_reads_without_that_field() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_str().unwrap();
    init_with_requirements(dir.path(), &[PLAIN, EXTENDED]);

    get(repo, "req_extended")
        .success()
        .stdout(contains("Records keep only known fields."))
        .stdout(contains("extension").not())
        .stdout(contains("newer-tool").not());
}

fn init_with_requirements(repo: &std::path::Path, rows: &[&str]) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
        ])
        .assert()
        .success();
    let path = repo.join(".provenance/state/scopes/default/requirements/req.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        rows.iter()
            .map(|row| format!("{row}\n"))
            .collect::<String>(),
    )
    .unwrap();
}

fn get(repo: &str, id: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([id, "get", "--repo", repo, "--format", "json"])
        .assert()
}
