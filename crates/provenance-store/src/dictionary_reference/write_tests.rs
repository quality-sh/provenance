//! The write of the dictionary reference: the directory has a sync after the
//! rename, a failed write keeps the stored reference, and no temporary file
//! stays.
//!
//! No test stops the process or the electrical supply: each test shows the
//! order of the steps of one write.

use super::{dictionary_reference_path, set_project_dictionary};
use crate::layout::ProvenanceLayout;
use crate::test_support::closed_directory::{close, Closed};
use crate::test_support::dictionary_import::import;
use crate::test_support::directory::entry_names;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::verifies;

/// A stored reference with a technical name that a write must not lose.
const STORED: &str = r#"{"issue": 9, "source_sha256": "stored", "data_sha256": "stored",
"extractor_version": "0.0.0", "technical_names": ["bword"]}"#;

/// A repository with its state directory, and the index directory beside it.
fn repository(temp: &tempfile::TempDir) -> (ProvenanceLayout, Utf8PathBuf, Utf8PathBuf) {
    let root = Utf8Path::from_path(temp.path()).expect("the temporary directory is UTF-8");
    let layout = ProvenanceLayout::new(root.join("repo"));
    let state = layout.state_dir();
    std::fs::create_dir_all(&state).expect("make the state directory");
    (layout, state, root.join("index"))
}

#[test]
#[verifies("rule_ste_dictionary_file_write_is_durable", examples)]
fn a_reference_write_syncs_the_state_directory_after_the_rename() {
    let temp = tempfile::tempdir().expect("make a temporary directory");
    let (layout, state, index) = repository(&temp);

    let closed = close(&state, Closed::Reads);
    let result = set_project_dictionary(&layout, &import(), index.as_std_path());
    drop(closed);

    let error = result.expect_err("the directory sync must fail");
    assert_eq!(
        format!("{error:#}"),
        format!(
            "write the dictionary reference: sync {state}: {}",
            std::io::Error::from_raw_os_error(13)
        )
    );
    let path = dictionary_reference_path(&layout);
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("read the reference"))
            .expect("the reference is JSON");
    assert_eq!(
        written,
        serde_json::to_value(&import().identity).expect("the identity is JSON")
    );
    assert_eq!(entry_names(&state), ["dictionary.json"]);
}

#[test]
#[verifies("rule_ste_dictionary_file_write_is_whole", examples)]
fn a_failed_write_of_the_reference_keeps_the_stored_reference() {
    let temp = tempfile::tempdir().expect("make a temporary directory");
    let (layout, state, index) = repository(&temp);
    let path = dictionary_reference_path(&layout);
    std::fs::write(&path, STORED).expect("store a reference");

    let closed = close(&state, Closed::Writes);
    let result = set_project_dictionary(&layout, &import(), index.as_std_path());
    drop(closed);

    let error = result.expect_err("the write of the temporary file must fail");
    assert!(
        format!("{error:#}").starts_with(&format!(
            "write the dictionary reference: make a temporary file for {path}: "
        )),
        "{error:#}"
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("read the reference"),
        STORED
    );
    assert_eq!(entry_names(&state), ["dictionary.json"]);
}
