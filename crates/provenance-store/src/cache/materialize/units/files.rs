//! The digests of each file of a unit, so a catch-up can tell which files a
//! write changed.

use super::{content_digest, frame, UnitBytes, UnitDigests, UnitHashError};
use camino::Utf8Path;
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
/// each file. The keys of `read` are below `state_dir`. `known_files` are
/// the file digests a previous pass stored: only a file whose stored digest
/// moved gets its content hashed again.
pub(in crate::cache::materialize) fn files_read(
    state_dir: &Utf8Path,
    read: &UnitBytes<'_>,
    known_files: &BTreeMap<String, FileDigest>,
) -> Result<UnitRead, UnitHashError> {
    let mut files = BTreeMap::new();
    let mut changed = BTreeSet::new();
    let mut content = Vec::new();
    for (key, bytes) in &read.entries {
        let stored = crate::canonical_digest::digest(bytes);
        let file = match known_files.get(*key) {
            Some(file) if file.stored == stored => file.clone(),
            _ => {
                changed.insert((*key).to_owned());
                // Component joins give the native spelling of the file path.
                let path = key
                    .split('/')
                    .fold(state_dir.to_path_buf(), |path, part| path.join(part));
                FileDigest {
                    content: content_digest(&path, bytes)?,
                    stored,
                }
            }
        };
        frame(&mut content, key, file.content.as_bytes());
        files.insert((*key).to_owned(), file);
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
