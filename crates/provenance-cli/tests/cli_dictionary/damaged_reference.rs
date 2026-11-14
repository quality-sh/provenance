//! The answers of the dictionary commands when the dictionary reference does
//! not parse, for example after a bad merge or a hand edit: an empty file, or a
//! file with half of a JSON object.
//!
//! The status and the statement check are implementation aids with no Rule:
//! they record what the product answers. The import and the init refuse, keep
//! the file and write no index, as `rule_ste_dictionary_reference_parse_refusal`
//! states. The strict check fails, as `rule_ste_strict_dictionary_index_gate`
//! states. The tests of an init with no PDF are in `cli_ste_onboarding`.

use crate::support::{
    dictionary_pdf, file_names, half_reference, init, init_command, provenance,
    reference_path, reference_problem, reference_refusal, HALF_REFERENCE_MESSAGE,
    REFERENCE_REPAIR, UNAPPROVED_WORD,
};
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Output;

/// A reference with no bytes.
const EMPTY: &str = "";

/// The parse message of [`EMPTY`].
const EMPTY_MESSAGE: &str = "EOF while parsing a value";

/// A repository whose dictionary reference holds `contents`, an empty index
/// directory, and the fixture PDF.
struct Damaged {
    _scratch: tempfile::TempDir,
    repo: PathBuf,
    index_directory: PathBuf,
    pdf: PathBuf,
}

impl Damaged {
    fn new(contents: &str) -> Self {
        let scratch = tempfile::tempdir().unwrap();
        let repo = scratch.path().join("repo");
        let index_directory = scratch.path().join("index");
        let pdf = scratch.path().join("dictionary.pdf");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&index_directory).unwrap();
        std::fs::write(&pdf, dictionary_pdf()).unwrap();
        init(&repo);
        std::fs::write(reference_path(&repo), contents).unwrap();
        Self {
            _scratch: scratch,
            repo,
            index_directory,
            pdf,
        }
    }

    fn repo(&self) -> &str {
        self.repo.to_str().unwrap()
    }

    fn run(&self, arguments: &[&str]) -> Output {
        provenance()
            .env("PROVENANCE_STE100_INDEX_DIR", &self.index_directory)
            .args(arguments)
            .output()
            .unwrap()
    }

    fn import(&self, extra: &[&str]) -> Output {
        let pdf = self.pdf.to_str().unwrap();
        let mut arguments = vec!["dictionary", "import", "--pdf", pdf, "--repo", self.repo()];
        arguments.extend_from_slice(extra);
        self.run(&arguments)
    }

    fn init(&self) -> Output {
        init_command(&self.repo, &["--ste-pdf", self.pdf.to_str().unwrap()])
            .env("PROVENANCE_STE100_INDEX_DIR", &self.index_directory)
            .output()
            .unwrap()
    }

    /// The exit code and standard error of a refused write, the bytes of the
    /// reference after it, and the names in the index directory.
    fn refusal(&self, output: &Output) -> (Option<i32>, String, String, Vec<String>) {
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
            std::fs::read_to_string(reference_path(&self.repo)).unwrap(),
            file_names(&self.index_directory),
        )
    }

    fn status(&self) -> (Option<i32>, Value, String) {
        answer(&self.run(&[
            "dictionary",
            "status",
            "--repo",
            self.repo(),
            "--format",
            "json",
        ]))
    }

    fn statement_check(&self, extra: &[&str]) -> (Option<i32>, Value, String) {
        let statement = format!("The {UNAPPROVED_WORD} item stops.");
        let mut arguments = vec!["check", "--repo", self.repo(), "--statement", &statement];
        arguments.extend_from_slice(extra);
        arguments.extend_from_slice(&["--format", "json"]);
        answer(&self.run(&arguments))
    }

    /// The status of a reference whose first line does not parse with
    /// `parse_error`.
    fn unloaded_status(&self, parse_error: &str) -> (Option<i32>, Value, String) {
        let status = json!({
            "referenced": true,
            "index_directory": self.index_directory.display().to_string(),
            "index_present": false,
            "analyzer": "built_in_rules",
            "load_problem": reference_problem(parse_error),
        });
        (Some(0), status, String::new())
    }
}

/// The statement check of a reference that does not parse with
/// `parse_error`: a warning that Rule 1.1 did not run, and no finding for
/// the unapproved word.
fn unloaded_check(parse_error: &str) -> (Option<i32>, Value, String) {
    let warning = format!(
        "ASD-STE100 Rule 1.1 did not run: {}",
        reference_problem(parse_error)
    );
    let check = json!({"categories": [{
        "category": "statements",
        "status": "findings",
        "findings": [{"message": warning}],
    }]});
    (Some(0), check, String::new())
}

/// The strict statement check of a reference that does not parse with
/// `parse_error`: the statements category cannot run, and the check fails.
fn unavailable_strict_check(parse_error: &str) -> (Option<i32>, Value, String) {
    let check = json!({"categories": [{
        "category": "statements",
        "status": "unavailable",
        "reason": reference_problem(parse_error),
    }]});
    (Some(1), check, String::new())
}

/// The refusal that a write gives for a reference whose first line does
/// not parse with `message`, the unchanged `contents`, and no index file.
fn refused(message: &str, contents: &str) -> (Option<i32>, String, String, Vec<String>) {
    (
        Some(1),
        reference_refusal(message),
        contents.to_owned(),
        Vec::new(),
    )
}

/// The refusal that an import with `--format json` gives, the unchanged
/// `contents`, and no index file.
fn refused_as_json(message: &str, contents: &str) -> (Option<i32>, String, String, Vec<String>) {
    let stderr = format!(
        r#"{{"error":{{"file":"dictionary.json","hint":"{REFERENCE_REPAIR}","kind":"invalid_state_file","line":1,"message":"{message}"}},"meta":{{}}}}"#
    );
    (Some(1), stderr + "\n", contents.to_owned(), Vec::new())
}

#[test]
fn the_status_of_an_empty_reference_names_the_parse_error() {
    let damaged = Damaged::new(EMPTY);

    assert_eq!(damaged.status(), damaged.unloaded_status(EMPTY_MESSAGE));
}

#[test]
fn the_status_of_half_of_a_reference_names_the_parse_error() {
    let damaged = Damaged::new(&half_reference());

    assert_eq!(
        damaged.status(),
        damaged.unloaded_status(HALF_REFERENCE_MESSAGE)
    );
}

#[test]
fn a_statement_check_with_an_empty_reference_warns_that_rule_1_1_did_not_run() {
    let damaged = Damaged::new(EMPTY);

    assert_eq!(damaged.statement_check(&[]), unloaded_check(EMPTY_MESSAGE));
}

#[test]
fn a_statement_check_with_half_of_a_reference_warns_that_rule_1_1_did_not_run() {
    let damaged = Damaged::new(&half_reference());

    assert_eq!(
        damaged.statement_check(&[]),
        unloaded_check(HALF_REFERENCE_MESSAGE)
    );
}

#[test]
#[verifies("rule_ste_strict_dictionary_index_gate", examples)]
fn a_strict_statement_check_with_half_of_a_reference_fails() {
    let damaged = Damaged::new(&half_reference());

    assert_eq!(
        damaged.statement_check(&["--strict"]),
        unavailable_strict_check(HALF_REFERENCE_MESSAGE)
    );
}

#[test]
#[verifies("rule_ste_dictionary_reference_parse_refusal", examples)]
fn an_import_over_an_empty_reference_refuses_and_keeps_the_file() {
    let damaged = Damaged::new(EMPTY);

    let import = damaged.import(&[]);

    assert_eq!(damaged.refusal(&import), refused(EMPTY_MESSAGE, EMPTY));
}

#[test]
#[verifies("rule_ste_dictionary_reference_parse_refusal", examples)]
fn an_import_over_half_of_a_reference_refuses_and_keeps_the_technical_names() {
    let damaged = Damaged::new(&half_reference());

    let import = damaged.import(&[]);

    assert_eq!(
        damaged.refusal(&import),
        refused(HALF_REFERENCE_MESSAGE, &half_reference())
    );
}

#[test]
#[verifies("rule_ste_dictionary_reference_parse_refusal", examples)]
fn an_init_over_an_empty_reference_refuses_and_keeps_the_file() {
    let damaged = Damaged::new(EMPTY);

    let init = damaged.init();

    assert_eq!(damaged.refusal(&init), refused(EMPTY_MESSAGE, EMPTY));
}

#[test]
#[verifies("rule_ste_dictionary_reference_parse_refusal", examples)]
fn an_init_over_half_of_a_reference_refuses_and_keeps_the_technical_names() {
    let damaged = Damaged::new(&half_reference());

    let init = damaged.init();

    assert_eq!(
        damaged.refusal(&init),
        refused(HALF_REFERENCE_MESSAGE, &half_reference())
    );
}

#[test]
#[verifies("rule_ste_dictionary_reference_parse_refusal", examples)]
fn an_import_with_json_output_over_an_empty_reference_gives_the_refusal_object() {
    let damaged = Damaged::new(EMPTY);

    let import = damaged.import(&["--format", "json"]);

    assert_eq!(
        damaged.refusal(&import),
        refused_as_json(EMPTY_MESSAGE, EMPTY)
    );
}

#[test]
#[verifies("rule_ste_dictionary_reference_parse_refusal", examples)]
fn an_import_with_json_output_over_half_of_a_reference_gives_the_refusal_object() {
    let damaged = Damaged::new(&half_reference());

    let import = damaged.import(&["--format", "json"]);

    assert_eq!(
        damaged.refusal(&import),
        refused_as_json(HALF_REFERENCE_MESSAGE, &half_reference())
    );
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
