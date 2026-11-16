//! The answers of the statement check when the dictionary reference file is
//! there and the process cannot read it, and when no reference file is there.
//!
//! A reference file that cannot be read is a reference that cannot load. The
//! statement check warns that Rule 1.1 did not run, and the strict check
//! stops with an error, as `rule_ste_dictionary_index_missing_warning` states.
//! The answer gives the path of the file below the repository and the system
//! error of the read. A reference file that is not there is no reference, and
//! the check gives no warning.

use super::support::{answer, init, provenance, reference_path, UNAPPROVED_WORD};
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::io::ErrorKind;
use std::path::PathBuf;

/// The text that Provenance writes before the system error of the read.
const UNREADABLE: &str =
    "Provenance cannot read the dictionary reference .provenance/state/dictionary.json";

/// An initialized repository with no dictionary reference file, and an empty
/// machine index directory.
struct Project {
    _scratch: tempfile::TempDir,
    repo: PathBuf,
    index_directory: PathBuf,
}

impl Project {
    fn new() -> Self {
        let scratch = tempfile::tempdir().unwrap();
        let repo = scratch.path().join("repo");
        let index_directory = scratch.path().join("index");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&index_directory).unwrap();
        init(&repo);
        match std::fs::remove_file(reference_path(&repo)) {
            Ok(()) => {}
            Err(error) => assert_eq!(error.kind(), ErrorKind::NotFound),
        }
        Self {
            _scratch: scratch,
            repo,
            index_directory,
        }
    }

    /// The statement check of a statement with an unapproved word, without
    /// and with the strict flag.
    fn statement_checks(&self) -> [(Option<i32>, Value, String); 2] {
        [&[][..], &["--strict"][..]].map(|extra| {
            let statement = format!("The {UNAPPROVED_WORD} item stops.");
            answer(
                &provenance()
                    .env("PROVENANCE_STE100_INDEX_DIR", &self.index_directory)
                    .args(["check", "--repo", self.repo.to_str().unwrap()])
                    .args(["--statement", &statement])
                    .args(extra)
                    .args(["--format", "json"])
                    .output()
                    .unwrap(),
            )
        })
    }

    /// The error of a read of the reference file by this process, when the
    /// read fails.
    fn read_error(&self) -> Option<std::io::Error> {
        std::fs::read(reference_path(&self.repo)).err()
    }
}

/// The answers for a reference file whose read fails with `system`: the
/// warning that Rule 1.1 did not run, and the strict refusal with the same
/// problem.
fn unreadable_checks(system: &std::io::Error) -> [(Option<i32>, Value, String); 2] {
    let problem = format!("{UNREADABLE}: {system}");
    let warned = json!({"categories": [{
        "category": "statements",
        "status": "findings",
        "findings": [{"message": format!("ASD-STE100 Rule 1.1 did not run: {problem}")}],
    }]});
    let refused = json!({"categories": [{
        "category": "statements",
        "status": "unavailable",
        "findings": [],
        "unavailable_reason": problem,
    }]});
    [
        (Some(0), warned, String::new()),
        (Some(1), refused, format!("Error: {problem}\n")),
    ]
}

/// The kind of the error of a read of a directory: Unix systems refuse it
/// as a directory, and Windows refuses access to it.
#[cfg(unix)]
const DIRECTORY_READ: ErrorKind = ErrorKind::IsADirectory;
#[cfg(windows)]
const DIRECTORY_READ: ErrorKind = ErrorKind::PermissionDenied;

#[test]
#[verifies("rule_ste_dictionary_index_missing_warning", examples)]
fn a_reference_path_that_is_a_directory_cannot_load() {
    let project = Project::new();
    std::fs::create_dir(reference_path(&project.repo)).unwrap();
    let system = project.read_error().expect("a read of a directory fails");
    assert_eq!(system.kind(), DIRECTORY_READ);

    assert_eq!(project.statement_checks(), unreadable_checks(&system));
}

/// A process of the root user reads a file with no read permission, so the
/// test stops when the read of the file does not fail.
#[cfg(unix)]
#[test]
#[verifies("rule_ste_dictionary_index_missing_warning", examples)]
fn a_reference_file_with_no_read_permission_cannot_load() {
    use std::os::unix::fs::PermissionsExt;
    let project = Project::new();
    let path = reference_path(&project.repo);
    std::fs::write(&path, b"{}").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    let Some(system) = project.read_error() else {
        return;
    };
    assert_eq!(system.kind(), ErrorKind::PermissionDenied);

    assert_eq!(project.statement_checks(), unreadable_checks(&system));
}

#[test]
#[verifies("rule_ste_dictionary_index_missing_warning", examples)]
fn a_reference_file_that_is_not_there_is_no_reference() {
    let project = Project::new();
    let passed = json!({"categories": [{
        "category": "statements",
        "status": "passed",
        "findings": [],
    }]});

    assert_eq!(
        project.statement_checks(),
        [
            (Some(0), passed.clone(), String::new()),
            (Some(0), passed, String::new()),
        ]
    );
}
