//! Verified extraction of the pinned review archive.
//!
//! The archive is a trusted build input for the embedded renderer. Extraction
//! verifies the committed SHA-256 before the destination exists and refuses
//! unsafe entries, so a changed or hostile archive cannot create files.

use std::{
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use tar::Archive;

/// Extract a SHA-256 verified archive into a new output directory.
pub fn extract_verified(archive_path: &Path, output: &Path, sha256: &str) -> io::Result<()> {
    let data = std::fs::read(archive_path)?;
    if hex_digest(&data) != sha256.to_ascii_lowercase() {
        return Err(io::Error::other("review archive checksum mismatch"));
    }
    let entries = verified_entries(&data)?;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Fails when the destination already exists.
    std::fs::create_dir(output)?;
    unpack(&data, output, &entries)
}

/// Check every entry before any output byte exists.
fn verified_entries(data: &[u8]) -> io::Result<Vec<(PathBuf, bool)>> {
    let mut entries = Vec::new();
    for entry in Archive::new(GzDecoder::new(data)).entries()? {
        let entry = entry?;
        let name = entry.path()?.to_str().ok_or_else(unsafe_entry)?.to_owned();
        let is_dir = entry.header().entry_type() == tar::EntryType::Directory;
        let path = PathBuf::from(&name);
        if name.starts_with('/')
            || name.contains('\\')
            || name.contains(':')
            || path
                .components()
                .any(|component| component == Component::ParentDir)
            || !matches!(
                entry.header().entry_type(),
                tar::EntryType::Regular | tar::EntryType::Directory
            )
        {
            return Err(unsafe_entry());
        }
        entries.push((path, is_dir));
    }
    Ok(entries)
}

fn unpack(data: &[u8], output: &Path, entries: &[(PathBuf, bool)]) -> io::Result<()> {
    for (index, entry) in Archive::new(GzDecoder::new(data)).entries()?.enumerate() {
        let mut entry = entry?;
        let name = entry.path()?.to_str().ok_or_else(unsafe_entry)?.to_owned();
        let (path, is_dir) = entries
            .get(index)
            .filter(|(record, _)| record == &PathBuf::from(&name))
            .ok_or_else(unsafe_entry)?;
        let target = output.join(path);
        if *is_dir {
            std::fs::create_dir_all(target)?;
        } else {
            std::fs::create_dir_all(target.parent().expect("non-root entry"))?;
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents)?;
            std::fs::write(target, contents)?;
        }
    }
    Ok(())
}

fn unsafe_entry() -> io::Error {
    io::Error::other("review archive contains an unsafe entry")
}

fn hex_digest(data: &[u8]) -> String {
    use std::fmt::Write;
    let mut text = String::with_capacity(64);
    for byte in Sha256::digest(data) {
        write!(&mut text, "{byte:02x}").expect("hex digit");
    }
    text
}
