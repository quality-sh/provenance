//! The write of the dictionary reference: the directory has a sync after the
//! rename, and no temporary file stays.
//!
//! No test stops the process or the electrical supply: each test shows the
//! order of the steps of one write.

use super::{dictionary_reference_path, set_project_dictionary};
use crate::layout::ProvenanceLayout;
use crate::test_support::closed_directory::{close, Closed};
use camino::Utf8Path;
use provenance_macros::verifies;
use provenance_ste100::{DictionaryImport, DictionaryImportIdentity, StandardIssue};

#[test]
#[verifies("rule_ste_dictionary_import_durable_writes", examples)]
fn a_reference_write_syncs_the_state_directory_after_the_rename() {
    let temp = tempfile::tempdir().expect("make a temporary directory");
    let root = Utf8Path::from_path(temp.path()).expect("the temporary directory is UTF-8");
    let layout = ProvenanceLayout::new(root.join("repo"));
    let state = layout.state_dir();
    std::fs::create_dir_all(&state).expect("make the state directory");
    let import = DictionaryImport {
        identity: DictionaryImportIdentity {
            issue: StandardIssue::Nine,
            source_sha256: "source".to_owned(),
            data_sha256: "data".to_owned(),
            extractor_version: "0.0.0".to_owned(),
        },
        entries: Vec::new(),
    };

    let closed = close(&state, Closed::Reads);
    let result = set_project_dictionary(&layout, &import, root.join("index").as_std_path());
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
        serde_json::to_value(&import.identity).expect("the identity is JSON")
    );
    let names: Vec<String> = std::fs::read_dir(&state)
        .expect("list the state directory")
        .map(|entry| {
            entry
                .expect("read an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names, ["dictionary.json"]);
}
