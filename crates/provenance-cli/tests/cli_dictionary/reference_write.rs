//! The write of the dictionary reference, seen through a state directory that
//! the process can write to and search but cannot open to read: the write
//! lands, and the open of the directory for its sync fails.
//!
//! The test is an implementation aid with no Rule, because no Rule states the
//! durability of the write of the dictionary reference.

use crate::support::{dictionary_pdf, imported_dictionary, init, provenance, reference_path};
use provenance_ste100::DictionaryImportIdentity;
use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[test]
fn a_dictionary_import_syncs_the_state_directory_after_the_reference_write() {
    let scratch = tempfile::tempdir().unwrap();
    let repo = scratch.path().join("repo");
    let index_directory = scratch.path().join("index");
    let pdf = scratch.path().join("dictionary.pdf");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(&pdf, dictionary_pdf()).unwrap();
    init(&repo);
    let reference = reference_path(&repo);
    let state = reference.parent().unwrap().to_owned();
    let mut expected_names = entry_names(&state);
    expected_names.insert("dictionary.json".to_owned());

    let Some(closed) = close_to_reads(&state) else {
        return;
    };
    let output = provenance()
        .env("PROVENANCE_STE100_INDEX_DIR", &index_directory)
        .env_remove("RUST_BACKTRACE")
        .env_remove("RUST_LIB_BACKTRACE")
        .args([
            "dictionary",
            "import",
            "--format",
            "json",
            "--pdf",
            pdf.to_str().unwrap(),
            "--repo",
            repo.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    drop(closed);

    assert_eq!(
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).into_owned()
        ),
        (
            Some(1),
            format!(
                "Error: write the dictionary reference\n\nCaused by:\n    0: sync {}\n    1: {}\n",
                state.display(),
                std::io::Error::from_raw_os_error(13)
            )
        ),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let written: DictionaryImportIdentity =
        serde_json::from_slice(&std::fs::read(&reference).unwrap()).unwrap();
    assert_eq!(written, imported_dictionary().identity);
    assert_eq!(
        entry_names(&state),
        expected_names,
        "no temporary file must stay"
    );
}

fn entry_names(directory: &Path) -> BTreeSet<String> {
    std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}

/// A directory that is closed to reads. The drop opens it again, so that a
/// failure or a panic does not keep it closed and the temporary directory
/// can be deleted.
struct ClosedDirectory(PathBuf);

impl Drop for ClosedDirectory {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

/// Closes `directory` to reads: the process can write to it and search it.
/// When the process can open it all the same, as root can, the test cannot
/// observe the sync: the function opens the directory again, prints a line
/// and gives nothing.
fn close_to_reads(directory: &Path) -> Option<ClosedDirectory> {
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o300)).unwrap();
    let closed = ClosedDirectory(directory.to_owned());
    if std::fs::File::open(directory).is_ok() {
        drop(closed);
        println!(
            "skipped: the process can open {} with no read permission",
            directory.display()
        );
        return None;
    }
    Some(closed)
}
