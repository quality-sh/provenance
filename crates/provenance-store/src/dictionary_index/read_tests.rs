//! A user reads these texts as the load problem of the dictionary status and in the
//! failure of a strict check. No Rule states the texts, so this test has no Rule
//! binding. It makes each fault of an index file that the store reads and shows its
//! text. `provenance-ste100` tests the texts of an identity, a digest and a structure
//! that do not match.

use super::load_dictionary_index;
use provenance_ste100::{dictionary_index_file_name, DictionaryImportIdentity, StandardIssue};

#[test]
fn each_fault_of_an_index_file_gives_its_diagnostic() {
    let temp = tempfile::tempdir().expect("make a temporary directory");
    let directory = temp.path();
    let identity = DictionaryImportIdentity {
        issue: StandardIssue::Nine,
        source_sha256: "source".to_owned(),
        data_sha256: "data".to_owned(),
        extractor_version: "0.0.0".to_owned(),
    };
    let path = directory.join(dictionary_index_file_name(&identity));
    let at = path.display();
    let load = || {
        load_dictionary_index(directory, &identity)
            .expect_err("the index has a fault")
            .to_string()
    };

    assert_eq!(load(), format!("no index file at {at}"));

    std::fs::create_dir(&path).expect("put a directory at the index path");
    let access = load();
    let cause = access.strip_prefix(&format!("index file access at {at}: "));
    assert!(cause.is_some_and(|cause| !cause.is_empty()), "{access}");
    std::fs::remove_dir(&path).expect("remove the directory at the index path");

    std::fs::write(&path, "{").expect("write a file that is not a JSON document");
    let malformed = load();
    let cause = malformed.strip_prefix(&format!("malformed index file at {at}: "));
    assert!(cause.is_some_and(|cause| !cause.is_empty()), "{malformed}");
}
