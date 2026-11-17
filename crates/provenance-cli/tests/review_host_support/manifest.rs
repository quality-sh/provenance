//! The state manifest of a test repository, and the writes that damage it.
use provenance_core::protocol::read_failure::STATE_CORRECTIONS;
use std::{
    io::Write as _,
    path::{Path, PathBuf},
    time::Duration,
};

/// A manifest that parses as JSON but has a string where the scope list must be.
pub const DAMAGED: &str = "{\"scopes\":\"default\"}\n";

/// The read fault that [`DAMAGED`] gives.
pub fn damaged_reason() -> String {
    format!(
        "manifest.json line 1: invalid type: string \"default\", expected a sequence; \
         {STATE_CORRECTIONS}"
    )
}

pub fn path(root: &Path) -> PathBuf {
    root.join(".provenance/state/manifest.json")
}

/// Saves `text` in place as an editor does: the file is empty for a short time before the
/// text is in it.
pub fn save_in_two_steps(path: &Path, text: &str) {
    let mut file = std::fs::File::create(path).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    file.write_all(text.as_bytes()).unwrap();
}
