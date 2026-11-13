//! The answers of the dictionary commands when the dictionary reference does
//! not parse, for example after a bad merge or a hand edit: an empty file, or a
//! file with half of a JSON object.
//!
//! The status and the statement check are implementation aids with no Rule:
//! they record what the product answers. The import and the init refuse and
//! keep the file, as `rule_ste_dictionary_reference_parse_refusal` states.

use crate::support::{
    dictionary_pdf, imported_dictionary, init, init_command, provenance, reference_path,
    UNAPPROVED_WORD,
};
use provenance_macros::verifies;
use provenance_ste100::store_dictionary_index;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Output;

/// A reference with no bytes.
const EMPTY: &str = "";

/// The parse message of [`EMPTY`].
const EMPTY_MESSAGE: &str = "EOF while parsing a value";

/// The parse message of [`half`].
const HALF_MESSAGE: &str = "EOF while parsing a string";

/// A reference that stops in the name of its third field. It holds a
/// technical name that a write must not lose.
fn half() -> String {
    format!(r#"{{"technical_names": ["{UNAPPROVED_WORD}"], "issue": 9, "source_sha"#)
}

/// A repository whose dictionary reference holds `contents`, the index
/// directory that holds the index of the fixture, and the fixture PDF.
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
        std::fs::write(&pdf, dictionary_pdf()).unwrap();
        init(&repo);
        store_dictionary_index(imported_dictionary(), &index_directory).unwrap();
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

    fn import(&self) -> Output {
        let pdf = self.pdf.to_str().unwrap();
        self.run(&["dictionary", "import", "--pdf", pdf, "--repo", self.repo()])
    }

    fn init(&self) -> Output {
        init_command(&self.repo, &["--ste-pdf", self.pdf.to_str().unwrap()])
            .env("PROVENANCE_STE100_INDEX_DIR", &self.index_directory)
            .output()
            .unwrap()
    }

    /// The exit code and standard error of a refused write, and the bytes of
    /// the reference after it.
    fn refusal(&self, output: &Output) -> (Option<i32>, String, String) {
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
            std::fs::read_to_string(reference_path(&self.repo)).unwrap(),
        )
    }

    /// The refusal that a write gives for a reference whose first line does
    /// not parse with `message`, and the unchanged `contents`.
    fn refused(&self, message: &str, contents: &str) -> (Option<i32>, String, String) {
        let stderr = format!(
            "Error: the dictionary reference {} does not parse: restore it from git or correct \
             its JSON, and then run the command again\n\nCaused by:\n    dictionary.json line \
             1: {message}\n",
            reference_path(&self.repo).display()
        );
        (Some(1), stderr, contents.to_owned())
    }

    fn status(&self) -> (Option<i32>, Value, String) {
        answer(&self.run(&["dictionary", "status", "--repo", self.repo(), "--format", "json"]))
    }

    fn statement_check(&self) -> (Option<i32>, Value, String) {
        let statement = format!("The {UNAPPROVED_WORD} item stops.");
        answer(&self.run(&[
            "check",
            "--repo",
            self.repo(),
            "--statement",
            &statement,
            "--format",
            "json",
        ]))
    }

    /// The status of a reference that does not parse with `parse_error`.
    fn unloaded_status(&self, parse_error: &str) -> (Option<i32>, Value, String) {
        let status = json!({
            "referenced": true,
            "index_directory": self.index_directory.display().to_string(),
            "index_present": false,
            "analyzer": "built_in_rules",
            "load_problem": problem(parse_error),
        });
        (Some(0), status, String::new())
    }

    /// The statement check of a reference that does not parse with
    /// `parse_error`: a warning that Rule 1.1 did not run, and no finding for
    /// the unapproved word.
    fn unloaded_check(&self, parse_error: &str) -> (Option<i32>, Value, String) {
        let warning = format!(
            "ASD-STE100 Rule 1.1 did not run: the dictionary reference has no loadable index in \
             {}: {}. Run `provenance dictionary import` with the local Issue 9 PDF on this \
             machine",
            self.index_directory.display(),
            problem(parse_error)
        );
        let check = json!({"categories": [{
            "category": "statements",
            "status": "findings",
            "findings": [{"message": warning}],
        }]});
        (Some(0), check, String::new())
    }
}

fn problem(parse_error: &str) -> String {
    format!("the reference is not a dictionary identity: {parse_error}")
}

#[test]
fn the_status_of_an_empty_reference_names_the_parse_error() {
    let damaged = Damaged::new(EMPTY);

    assert_eq!(
        damaged.status(),
        damaged.unloaded_status("EOF while parsing a value at line 1 column 0")
    );
}

#[test]
fn the_status_of_half_of_a_reference_names_the_parse_error() {
    let damaged = Damaged::new(&half());

    assert_eq!(
        damaged.status(),
        damaged.unloaded_status("EOF while parsing a string at line 1 column 63")
    );
}

#[test]
fn a_statement_check_with_an_empty_reference_warns_that_rule_1_1_did_not_run() {
    let damaged = Damaged::new(EMPTY);

    assert_eq!(
        damaged.statement_check(),
        damaged.unloaded_check("EOF while parsing a value at line 1 column 0")
    );
}

#[test]
fn a_statement_check_with_half_of_a_reference_warns_that_rule_1_1_did_not_run() {
    let damaged = Damaged::new(&half());

    assert_eq!(
        damaged.statement_check(),
        damaged.unloaded_check("EOF while parsing a string at line 1 column 63")
    );
}

#[test]
#[verifies("rule_ste_dictionary_reference_parse_refusal", examples)]
fn an_import_over_an_empty_reference_refuses_and_keeps_the_file() {
    let damaged = Damaged::new(EMPTY);

    let import = damaged.import();

    assert_eq!(
        damaged.refusal(&import),
        damaged.refused(EMPTY_MESSAGE, EMPTY)
    );
}

#[test]
#[verifies("rule_ste_dictionary_reference_parse_refusal", examples)]
fn an_import_over_half_of_a_reference_refuses_and_keeps_the_technical_names() {
    let damaged = Damaged::new(&half());

    let import = damaged.import();

    assert_eq!(
        damaged.refusal(&import),
        damaged.refused(HALF_MESSAGE, &half())
    );
}

#[test]
#[verifies("rule_ste_dictionary_reference_parse_refusal", examples)]
fn an_init_over_an_empty_reference_refuses_and_keeps_the_file() {
    let damaged = Damaged::new(EMPTY);

    let init = damaged.init();

    assert_eq!(
        damaged.refusal(&init),
        damaged.refused(EMPTY_MESSAGE, EMPTY)
    );
}

#[test]
#[verifies("rule_ste_dictionary_reference_parse_refusal", examples)]
fn an_init_over_half_of_a_reference_refuses_and_keeps_the_technical_names() {
    let damaged = Damaged::new(&half());

    let init = damaged.init();

    assert_eq!(
        damaged.refusal(&init),
        damaged.refused(HALF_MESSAGE, &half())
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

