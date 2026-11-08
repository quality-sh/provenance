//! The digests of each file of a unit, so a catch-up can tell which files a
//! write changed.

use super::{content_digest, frame, UnitBytes, UnitDigests, UnitHashError};
use std::collections::{BTreeMap, BTreeSet};

/// The stored digest and the content digest of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::cache::materialize) struct FileDigest {
    pub stored: String,
    pub content: String,
}

/// One read of a unit. It holds the digests of the unit, and the digests of
/// each of its files by path below the state directory. It also holds the
/// paths whose bytes differ from the files that a previous pass stored.
/// These include the added and the removed files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::cache::materialize) struct UnitRead {
    pub digests: UnitDigests,
    pub files: BTreeMap<String, FileDigest>,
    pub changed: BTreeSet<String>,
}

/// The digests of one unit whose bytes are `read`, with the digests of
/// each file. `known_files` are the file digests a previous pass stored:
/// only a file whose stored digest moved gets its content hashed again.
pub(in crate::cache::materialize) fn files_read(
    read: &UnitBytes,
    known_files: &BTreeMap<String, FileDigest>,
) -> Result<UnitRead, UnitHashError> {
    let mut files = BTreeMap::new();
    let mut changed = BTreeSet::new();
    let mut content = Vec::new();
    for ((relative, path), bytes) in read.files.iter().zip(&read.contents) {
        let stored = crate::canonical_digest::digest(bytes);
        let file = match known_files.get(relative) {
            Some(file) if file.stored == stored => file.clone(),
            _ => {
                changed.insert(relative.clone());
                FileDigest {
                    content: content_digest(path, bytes)?,
                    stored,
                }
            }
        };
        frame(&mut content, relative, file.content.as_bytes());
        files.insert(relative.clone(), file);
    }
    changed.extend(
        known_files
            .keys()
            .filter(|relative| !files.contains_key(*relative))
            .cloned(),
    );
    Ok(UnitRead {
        digests: UnitDigests {
            content: crate::canonical_digest::digest(&content),
            stored: read.stored.clone(),
        },
        files,
        changed,
    })
}
