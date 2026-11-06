//! Implementation aid with no Rule binding: a stored record has one format and
//! carries no format field. The tests read hand-written rows through
//! `provenance <id> get`, so they check the stored format and not one writer.
use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

const PLAIN: &str = r#"{"scope_id":"default","id":"req_plain","statement":"Records have one stored format.","status":"active"}"#;
const EXTENDED: &str = r#"{"scope_id":"default","id":"req_extended","statement":"Records hold only known fields.","status":"active","extension":{"owner":"newer-tool"}}"#;

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

/// A graph record family holds only the fields of its record type, so the
/// store refuses a stored line with a field that the type does not hold.
#[test]
fn a_record_with_an_unknown_field_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_str().unwrap();
    init_with_requirements(dir.path(), &[PLAIN, EXTENDED]);

    get(repo, "req_plain")
        .failure()
        .stdout(contains("Records have one stored format.").not())
        .stderr(contains("invalid requirement").or(contains("extension")));
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
    std::fs::write(path, rows.join("\n") + "\n").unwrap();
}

fn get(repo: &str, id: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([id, "get", "--repo", repo, "--format", "json"])
        .assert()
}
