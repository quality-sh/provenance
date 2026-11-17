//! The error text of a write of one state key whose rename fails because the
//! cache and the state directory are on two file systems.
//!
//! A test cannot make a second file system below a repository without the
//! rights of an administrator, so this test gives the error of the rename to
//! the function that makes the text.

use super::rename_failure;
use crate::layout::ProvenanceLayout;

// No Rule: no Rule states the error text of a write to the local disk.
#[test]
fn a_rename_across_two_file_systems_names_the_cache_and_the_state_directory() {
    let layout = ProvenanceLayout::new("repo");
    let path = layout.state_dir().join("dictionary.json");
    let cross = std::io::Error::from(std::io::ErrorKind::CrossesDevices);
    let text = cross.to_string();

    let error = rename_failure(&layout, &path, cross);

    assert_eq!(
        format!("{error:#}"),
        format!(
            "rename a temporary file from the cache {} to {path}: the cache and the state \
             directory {} are on two file systems, and the store writes the state only when \
             they are on one file system. Put .provenance/cache on the file system of \
             .provenance/state, and then send the call again: {text}",
            layout.cache_dir(),
            layout.state_dir()
        )
    );
}
