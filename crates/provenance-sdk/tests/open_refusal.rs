//! The open of a repository whose manifest does not parse: the open failure
//! keeps the fault of the manifest as its cause, with the file, the line and
//! the two procedures that correct the state.

use provenance_core::protocol::read_failure::{ReadFailure, UnparsableStateFile, STATE_CORRECTIONS};
use provenance_macros::verifies;
use provenance_sdk::{OpenFailure, Repository};

#[test]
#[verifies("rule_state_parse_refusal_names_the_corrections", examples)]
fn the_open_failure_of_a_manifest_that_does_not_parse_has_the_fault_as_its_cause() {
    let directory = tempfile::tempdir().unwrap();
    let state = directory.path().join(".provenance/state");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::write(state.join("manifest.json"), "{\"scopes\":\"default\"}\n").unwrap();

    let error = Repository::open(directory.path(), "default").unwrap_err();
    assert!(matches!(error, OpenFailure::Io(_)), "{error:?}");
    let fault = UnparsableStateFile::find(&error).map(UnparsableStateFile::failure);
    assert_eq!(
        fault,
        Some(ReadFailure::InvalidStateFile {
            file: "manifest.json".to_owned(),
            line: 1,
            message: "invalid type: string \"default\", expected a sequence".to_owned(),
            hint: Some(STATE_CORRECTIONS.to_owned()),
        })
    );
}
