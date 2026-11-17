//! The write of the dictionary reference: the directory has a sync after the
//! rename, a failed write keeps the stored reference and names the directory
//! that refused it, and no temporary file stays.
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

    let error = result.expect_err("the rename into the state directory must fail");
    assert_eq!(
        format!("{error:#}"),
        format!(
            "write the dictionary reference: rename a temporary file to {path}: {}",
            std::io::Error::from_raw_os_error(13)
        )
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("read the reference"),
        STORED
    );
    assert_eq!(entry_names(&state), ["dictionary.json"]);
    let cache = layout.cache_dir();
    let temporaries = entry_names(&cache)
        .into_iter()
        .filter(|name| std::path::Path::new(name).extension() == Some("tmp".as_ref()))
        .collect::<Vec<_>>();
    assert_eq!(temporaries, Vec::<String>::new());
}

// No Rule: no Rule states the error text of a write to the local disk.
#[test]
fn a_cache_that_refuses_the_temporary_file_is_named_in_the_error() {
    let temp = tempfile::tempdir().expect("make a temporary directory");
    let (layout, state, index) = repository(&temp);
    let path = dictionary_reference_path(&layout);
    std::fs::write(&path, STORED).expect("store a reference");
    // An entry to the section makes the cache and the directories in it.
    crate::publication::with_repository_publication(&layout, |_| Ok(()))
        .expect("enter the section");
    let cache = layout.cache_dir();

    let closed = close(&cache, Closed::Writes);
    let result = set_project_dictionary(&layout, &import(), index.as_std_path());
    drop(closed);

    let error = result.expect_err("the temporary file in the cache must fail");
    // The I/O error of `tempfile` ends with the path of the file it tried.
    let text = format!("{error:#}");
    let start = format!(
        "write the dictionary reference: make a temporary file in {cache}: {} at path \"",
        std::io::Error::from_raw_os_error(13)
    );
    let tried = text
        .strip_prefix(&start)
        .and_then(|rest| rest.strip_suffix('"'))
        .map_or_else(|| panic!("{text}"), Utf8Path::new);
    assert_eq!(tried.parent(), Some(cache.as_path()), "{text}");
    assert!(
        tried
            .file_name()
            .is_some_and(crate::storage::local::is_state_temporary_name),
        "{text}"
    );
    assert_eq!(
        std::fs::read_to_string(&path).expect("read the reference"),
        STORED
    );
    assert_eq!(entry_names(&state), ["dictionary.json"]);
}

// No Rule: the order of the directory step is a step of the write; the Rules
// state the files that a write leaves.
#[test]
fn a_write_that_stops_before_its_rename_makes_no_state_directory() {
    let temp = tempfile::tempdir().expect("make a temporary directory");
    let root = Utf8Path::from_path(temp.path()).expect("the temporary directory is UTF-8");
    let layout = ProvenanceLayout::new(root.join("repo"));
    std::fs::create_dir_all(layout.root()).expect("make the repository");

    // The index write passes the probe first; the write of the reference
    // stops at it.
    let mut index_written = false;
    crate::test_probes::arm("synced_file_before_rename", move || {
        anyhow::ensure!(!std::mem::replace(&mut index_written, true), "stopped");
        Ok(())
    });
    let result = set_project_dictionary(&layout, &import(), root.join("index").as_std_path());
    crate::test_probes::disarm("synced_file_before_rename");

    let error = result.expect_err("the stop before the rename must stop the write");
    assert_eq!(
        format!("{error:#}"),
        "write the dictionary reference: stopped"
    );
    assert!(
        !layout.state_dir().exists(),
        "the stop made the state directory"
    );
}
