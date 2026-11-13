//! The answers of the dictionary commands when a stop during the write of the
//! dictionary reference left it damaged: an empty file, or a file with half of
//! a JSON object.
//!
//! Each test is an implementation aid with no Rule: it records the behaviour
//! of the product, and no Rule states it.

use crate::support::{
    dictionary_pdf, imported_dictionary, init, provenance, reference_path, APPROVED_TABLE_ROWS,
    UNAPPROVED_TABLE_ROWS, UNAPPROVED_WORD,
};
use provenance_ste100::store_dictionary_index;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Output;

#[test]
fn the_dictionary_commands_with_an_empty_reference() {
    record_the_answers("", "EOF while parsing a value at line 1 column 0");
}

#[test]
fn the_dictionary_commands_with_half_of_a_reference() {
    let half = format!(r#"{{"technical_names": ["{UNAPPROVED_WORD}"], "issue": 9, "source_sha"#);

    record_the_answers(&half, "EOF while parsing a string at line 1 column 63");
}

/// Runs the status, a statement check and an import, in this order, on a
/// repository whose reference holds `contents`, and compares each answer.
fn record_the_answers(contents: &str, parse_error: &str) {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    let index_directory = scratch.path().join("index");
    let pdf = scratch.path().join("dictionary.pdf");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(&pdf, dictionary_pdf()).unwrap();
    init(&repo);
    store_dictionary_index(imported_dictionary(), &index_directory).unwrap();
    std::fs::write(reference_path(&repo), contents).unwrap();
    let problem = format!("the reference is not a dictionary identity: {parse_error}");

    assert_the_status(&repo, &index_directory, &problem);
    assert_the_statement_check(&repo, &index_directory, &problem);
    assert_the_import(&repo, &index_directory, &pdf);
}

fn assert_the_status(repo: &Path, index_directory: &Path, problem: &str) {
    let status = run(
        index_directory,
        &[
            "dictionary",
            "status",
            "--repo",
            repo.to_str().unwrap(),
            "--format",
            "json",
        ],
    );
    assert_eq!(
        answer(&status),
        (
            Some(0),
            json!({
                "referenced": true,
                "index_directory": index_directory.display().to_string(),
                "index_present": false,
                "analyzer": "built_in_rules",
                "load_problem": problem,
            }),
            String::new()
        )
    );
}

fn assert_the_statement_check(repo: &Path, index_directory: &Path, problem: &str) {
    let statement = format!("The {UNAPPROVED_WORD} item stops.");
    let check = run(
        index_directory,
        &[
            "check",
            "--repo",
            repo.to_str().unwrap(),
            "--statement",
            &statement,
            "--format",
            "json",
        ],
    );
    let warning = format!(
        "ASD-STE100 Rule 1.1 did not run: the dictionary reference has no loadable index in \
         {}: {problem}. Run `provenance dictionary import` with the local Issue 9 PDF on \
         this machine",
        index_directory.display()
    );
    assert_eq!(
        answer(&check),
        (
            Some(0),
            json!({"categories": [{
                "category": "statements",
                "status": "findings",
                "findings": [{"message": warning}],
            }]}),
            String::new()
        )
    );
}

fn assert_the_import(repo: &Path, index_directory: &Path, pdf: &Path) {
    let import = run(
        index_directory,
        &[
            "dictionary",
            "import",
            "--format",
            "json",
            "--pdf",
            pdf.to_str().unwrap(),
            "--repo",
            repo.to_str().unwrap(),
        ],
    );
    let identity = &imported_dictionary().identity;
    assert_eq!(
        answer(&import),
        (
            Some(0),
            json!({
                "issue": 9,
                "source_sha256": identity.source_sha256,
                "data_sha256": identity.data_sha256,
                "extractor_version": identity.extractor_version,
                "approved_rows": APPROVED_TABLE_ROWS,
                "unapproved_rows": UNAPPROVED_TABLE_ROWS,
            }),
            String::new()
        )
    );
    let written: Value =
        serde_json::from_slice(&std::fs::read(reference_path(repo)).unwrap()).unwrap();
    assert_eq!(
        written,
        serde_json::to_value(identity).unwrap(),
        "the import writes the identity with no technical names"
    );
}

fn run(index_directory: &Path, arguments: &[&str]) -> Output {
    provenance()
        .env("PROVENANCE_STE100_INDEX_DIR", index_directory)
        .args(arguments)
        .output()
        .unwrap()
}

/// The exit code, the JSON on standard output and the text on standard error.
fn answer(output: &Output) -> (Option<i32>, Value, String) {
    let stdout = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout must be JSON: {error}: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    (
        output.status.code(),
        stdout,
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}
