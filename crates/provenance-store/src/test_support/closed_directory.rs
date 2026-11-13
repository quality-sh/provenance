//! A directory that the process cannot open to read or cannot write to, so
//! that a test sees the sync of a directory or a failed write in it.

use camino::{Utf8Path, Utf8PathBuf};
use std::os::unix::fs::PermissionsExt;

/// What a closed directory refuses to the process.
#[derive(Clone, Copy, Debug)]
pub enum Closed {
    /// An open to read, as the sync of a directory does. The process can
    /// still write to the directory and search it.
    Reads,
    /// A new entry. The process can still read the directory and search it.
    Writes,
}

/// A directory that is closed. The drop opens it again, so that a failure or
/// a panic does not keep it closed and the test can delete it.
pub struct ClosedDirectory(Utf8PathBuf);

impl Drop for ClosedDirectory {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

/// Closes `directory` as `closed` gives. A process that the permissions do
/// not stop, as root, cannot see the step that the test examines, so the
/// function panics and the test fails.
pub fn close(directory: &Utf8Path, closed: Closed) -> ClosedDirectory {
    let mode = match closed {
        Closed::Reads => 0o300,
        Closed::Writes => 0o500,
    };
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(mode))
        .unwrap_or_else(|error| panic!("close {directory}: {error}"));
    let guard = ClosedDirectory(directory.to_owned());
    let refused = match closed {
        Closed::Reads => std::fs::File::open(directory).is_err(),
        Closed::Writes => {
            let probe = directory.join("probe");
            let created = std::fs::File::create(&probe).is_ok();
            let _ = std::fs::remove_file(&probe);
            !created
        }
    };
    assert!(
        refused,
        "the permissions of {directory} do not stop this process, so the test cannot see the \
         step that it examines: run the test as a user that is not root"
    );
    guard
}
