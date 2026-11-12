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
    let dictionary = imported_dictionary();
    store_dictionary_index(dictionary, &index_directory).unwrap();
    std::fs::write(reference_path(&repo), contents).unwrap();
    let index = index_directory.display().to_string();
    let problem = format!("the reference is not a dictionary identity: {parse_error}");
    let repo_argument = repo.to_str().unwrap();

    let status = run(
        &index_directory,
        &[
            "dictionary",
            "status",
            "--repo",
            repo_argument,
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
                "index_directory": index,
                "index_present": false,
                "analyzer": "built_in_rules",
                "load_problem": problem,
            }),
            String::new()
        )
    );

    let statement = format!("The {UNAPPROVED_WORD} item stops.");
    let check = run(
        &index_directory,
        &[
            "check",
            "--repo",
            repo_argument,
            "--statement",
            &statement,
            "--format",
            "json",
        ],
    );
    let warning = format!(
        "ASD-STE100 Rule 1.1 did not run: the dictionary reference has no loadable index in \
         {index}: {problem}. Run `provenance dictionary import` with the local Issue 9 PDF on \
         this machine"
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

    let import = run(
        &index_directory,
        &[
            "dictionary",
            "import",
            "--format",
            "json",
            "--pdf",
            pdf.to_str().unwrap(),
            "--repo",
            repo_argument,
        ],
    );
    let identity = &dictionary.identity;
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
        serde_json::from_slice(&std::fs::read(reference_path(&repo)).unwrap()).unwrap();
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
