//! The committed pin and authenticated download for the renderer archive.

use std::{io, path::Path, process::Command};

use serde::Deserialize;

/// The committed pin for the supplied renderer archive.
#[derive(Deserialize)]
pub struct Pin {
    pub repository: String,
    pub commit: String,
    pub run: u64,
    pub artifact: String,
    pub archive: String,
    pub sha256: String,
}

/// Read the committed pin.
pub fn pin() -> Pin {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/review-assets.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("read review asset pin"))
        .expect("valid review asset pin")
}

/// Prepare the pinned renderer assets, downloading with `gh` when needed.
pub fn prepare(output: &Path, archive_path: Option<&Path>, pin: &Pin) -> io::Result<()> {
    if let Some(archive_path) = archive_path {
        return crate::archive::extract_verified(archive_path, output, &pin.sha256);
    }
    let temporary = tempfile::tempdir().map_err(|error| io::Error::other(error.to_string()))?;
    let status = Command::new("gh")
        .args(["run", "download", &pin.run.to_string()])
        .args(["--repo", &pin.repository])
        .args(["--name", &pin.artifact])
        .args(["--dir"])
        .arg(temporary.path())
        .status()
        .map_err(|error| io::Error::other(format!("cannot run gh: {error}")))?;
    if !status.success() {
        return Err(io::Error::other(
            "gh run download failed for the pinned review archive",
        ));
    }
    crate::archive::extract_verified(&temporary.path().join(&pin.archive), output, &pin.sha256)
}
