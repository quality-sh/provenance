//! Hash units: one per manifest scope and one global unit.
//!
//! The units cover canonical state: the manifest and each shard of a scope.
//! A scope unit is the keys below the scope's directory; the global unit is
//! every other key. Catch-up hashes one view, and the reads with no lock
//! hash the files at the same keys, so a stray file under `state/` moves no
//! digest. A digest frames each key and its bytes in key order, so two
//! shards that share a basename cannot swap contents unnoticed.

mod content;

use crate::storage::StateView;
use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{Manifest, ScopeId};

/// One hash unit. `name` is the key of its digest row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unit {
    Global,
    Scope(ScopeId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UnitDigests {
    pub content: String,
    pub stored: String,
}

impl Unit {
    pub fn name(&self) -> String {
        match self {
            Self::Global => "global".to_string(),
            Self::Scope(scope) => format!("scope:{}", scope.as_str()),
        }
    }

    /// The scope a stored unit name belongs to, if it is a scope unit.
    pub fn scope_of(name: &str) -> anyhow::Result<Option<ScopeId>> {
        match name.strip_prefix("scope:") {
            Some(scope) => Ok(Some(ScopeId::new(scope)?)),
            None => Ok(None),
        }
    }
}

/// Reads scope ids without the publication lock.
///
/// A manifest write renames the old file away before it renames the new one
/// into place, so an unlocked reader sees one complete version or, for the
/// width of those two renames, no file at all. A missing file is retried
/// rather than refused; the bytes a read returns are always one whole version.
pub fn scope_ids(state_dir: &Utf8Path) -> anyhow::Result<Vec<ScopeId>> {
    let path = state_dir.join("manifest.json");
    let bytes = read_through_rename(&path)?;
    let manifest: Manifest =
        serde_json::from_slice(&bytes).with_context(|| format!("parse manifest {path}"))?;
    Ok(manifest.scopes.into_iter().map(|scope| scope.id).collect())
}

/// How many times a lock-free manifest read waits out a rename window.
const MANIFEST_READ_ATTEMPTS: u32 = 8;

fn read_through_rename(path: &Utf8Path) -> anyhow::Result<Vec<u8>> {
    for attempt in 1..=MANIFEST_READ_ATTEMPTS {
        match std::fs::read(path) {
            Ok(bytes) => return Ok(bytes),
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && attempt < MANIFEST_READ_ATTEMPTS =>
            {
                std::thread::sleep(std::time::Duration::from_millis(u64::from(attempt) * 5));
            }
            Err(error) => {
                return Err(anyhow::Error::new(error).context(format!("read manifest {path}")))
            }
        }
    }
    unreachable!("the loop returns on its last attempt")
}

/// Every unit a manifest names, global first, scopes sorted.
pub fn units_for(scopes: &[ScopeId]) -> Vec<Unit> {
    let mut sorted: Vec<&ScopeId> = scopes.iter().collect();
    sorted.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    let mut units = vec![Unit::Global];
    units.extend(sorted.into_iter().map(|scope| Unit::Scope(scope.clone())));
    units
}

/// The digest of one unit's canonical bytes under `state_dir`, read from
/// the files with no lock.
pub fn unit_digest(state_dir: &Utf8Path, unit: &Unit) -> Result<String, UnitHashError> {
    Ok(digests_with(state_dir, unit, |_, _| {})?.content)
}

/// The digest of the exact stored bytes in one unit, read from the files
/// with no lock.
pub fn unit_stored_digest(state_dir: &Utf8Path, unit: &Unit) -> Result<String, UnitHashError> {
    Ok(read_unit(state_dir, unit, |_, _| {})?.stored)
}

/// The digests of one unit, read from the files. `retain` sees each file as
/// it is read.
pub(super) fn digests_with(
    state_dir: &Utf8Path,
    unit: &Unit,
    retain: impl FnMut(&Utf8Path, &[u8]),
) -> Result<UnitDigests, UnitHashError> {
    let read = read_unit(state_dir, unit, retain)?;
    let entries: Vec<(&str, &[u8])> = read
        .files
        .iter()
        .zip(&read.contents)
        .map(|((key, _), bytes)| (key.as_str(), bytes.as_slice()))
        .collect();
    let content = content_digest(state_dir, &entries)?;
    Ok(UnitDigests {
        content,
        stored: read.stored,
    })
}

/// The canonical files of one unit, their bytes, and the digest of those
/// exact bytes.
struct UnitBytes {
    files: Vec<(String, Utf8PathBuf)>,
    contents: Vec<Vec<u8>>,
    stored: String,
}

/// Reads the canonical files of one unit with no lock. A file list that
/// changes while the bytes are read refuses, so the bytes are one listing.
fn read_unit(
    state_dir: &Utf8Path,
    unit: &Unit,
    mut retain: impl FnMut(&Utf8Path, &[u8]),
) -> Result<UnitBytes, UnitHashError> {
    let files = unit_files(state_dir, unit)?;
    crate::test_probes::at("unit_files_collected")
        .map_err(|error| UnitHashError::at(state_dir, error))?;
    let mut contents = Vec::with_capacity(files.len());
    for (_, path) in &files {
        let bytes = std::fs::read(path).map_err(|error| UnitHashError::at(path, error))?;
        retain(path, &bytes);
        contents.push(bytes);
    }
    ensure_file_list_unchanged(state_dir, unit, &files)?;
    let entries: Vec<(&str, &[u8])> = files
        .iter()
        .zip(&contents)
        .map(|((key, _), bytes)| (key.as_str(), bytes.as_slice()))
        .collect();
    let stored = stored_digest(&entries);
    Ok(UnitBytes {
        files,
        contents,
        stored,
    })
}

fn ensure_file_list_unchanged(
    state_dir: &Utf8Path,
    unit: &Unit,
    files: &[(String, Utf8PathBuf)],
) -> Result<(), UnitHashError> {
    let after = unit_files(state_dir, unit)?;
    if files == after {
        return Ok(());
    }
    let changed = files
        .iter()
        .find(|entry| after.binary_search(entry).is_err())
        .or_else(|| {
            after
                .iter()
                .find(|entry| files.binary_search(entry).is_err())
        })
        .expect("different file lists have a changed entry");
    Err(UnitHashError::at(
        &changed.1,
        anyhow::anyhow!("canonical file list changed during hashing"),
    ))
}

/// The canonical files of one unit, by key: the keys that the view of the
/// local adapter holds.
fn unit_files(
    state_dir: &Utf8Path,
    unit: &Unit,
) -> Result<Vec<(String, Utf8PathBuf)>, UnitHashError> {
    let mut files = Vec::new();
    match unit {
        Unit::Global => collect(state_dir, "", true, &mut files)?,
        Unit::Scope(scope) => {
            let key = format!("scopes/{}", scope.as_str());
            let root = state_dir.join(&key);
            match std::fs::symlink_metadata(&root) {
                Ok(_) => collect(&root, &key, false, &mut files)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(UnitHashError::at(&root, error)),
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Collects the canonical files below `dir`, whose key is `prefix`. An
/// entry at a canonical key that is not a regular file refuses.
fn collect(
    dir: &Utf8Path,
    prefix: &str,
    skip_scopes: bool,
    out: &mut Vec<(String, Utf8PathBuf)>,
) -> Result<(), UnitHashError> {
    let metadata = std::fs::symlink_metadata(dir).map_err(|error| UnitHashError::at(dir, error))?;
    if !metadata.is_dir() {
        return Err(UnitHashError::at(
            dir,
            anyhow::anyhow!("unsupported state entry: {dir}"),
        ));
    }
    for entry in std::fs::read_dir(dir).map_err(|error| UnitHashError::at(dir, error))? {
        let entry = entry.map_err(|error| UnitHashError::at(dir, error))?;
        let path = Utf8PathBuf::from_path_buf(entry.path()).map_err(|path| {
            UnitHashError::at(
                dir,
                anyhow::anyhow!("non-UTF-8 canonical path: {}", path.display()),
            )
        })?;
        let name = path.file_name().unwrap_or_default();
        let key = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}/{name}")
        };
        let file_type = entry
            .file_type()
            .map_err(|error| UnitHashError::at(&path, error))?;
        if file_type.is_dir() {
            if !(skip_scopes && key == "scopes") && crate::shards::can_hold_canonical_keys(&key) {
                collect(&path, &key, skip_scopes, out)?;
            }
        } else if crate::shards::is_canonical_key(&key) {
            if !file_type.is_file() {
                return Err(UnitHashError::at(
                    &path,
                    anyhow::anyhow!("unsupported state entry: {path}"),
                ));
            }
            out.push((key, path));
        }
    }
    Ok(())
}

/// The digests of one unit of `view`, the canonical state under `state_dir`.
/// `known` is the pair a previous pass stored for the unit: when the stored
/// bytes still hash to its stored digest, the bytes have not changed and
/// neither has their content, so the pair is returned without a parse of
/// the records.
pub(super) fn unit_digests(
    state_dir: &Utf8Path,
    view: &StateView,
    unit: &Unit,
    known: Option<&UnitDigests>,
) -> Result<UnitDigests, UnitHashError> {
    let entries = unit_entries(view, unit);
    let stored = stored_digest(&entries);
    if let Some(known) = known.filter(|known| known.stored == stored) {
        return Ok(known.clone());
    }
    Ok(UnitDigests {
        content: content_digest(state_dir, &entries)?,
        stored,
    })
}

/// The digest of the record content of `entries`, without the Git creation
/// and update metadata.
fn content_digest(
    state_dir: &Utf8Path,
    entries: &[(&str, &[u8])],
) -> Result<String, UnitHashError> {
    let mut content = Vec::new();
    for (key, bytes) in entries {
        let path = state_dir.join(key);
        let content_bytes =
            content::hash_bytes(&path, bytes).map_err(|error| UnitHashError::at(&path, error))?;
        frame(&mut content, key, &content_bytes);
    }
    Ok(crate::canonical_digest::digest(&content))
}

/// The keys of `unit` in `view` and their bytes, in key order.
fn unit_entries<'v>(view: &'v StateView, unit: &Unit) -> Vec<(&'v str, &'v [u8])> {
    let scope = match unit {
        Unit::Global => None,
        Unit::Scope(scope) => Some(format!("scopes/{}/", scope.as_str())),
    };
    view.entries()
        .iter()
        .map(|(key, bytes)| (key.as_str(), bytes.as_slice()))
        .filter(|(key, _)| {
            scope.as_ref().map_or_else(
                || !key.starts_with("scopes/"),
                |prefix| key.starts_with(prefix.as_str()),
            )
        })
        .collect()
}

/// The digest of the exact bytes of `entries`.
fn stored_digest(entries: &[(&str, &[u8])]) -> String {
    let mut stored = Vec::new();
    for (key, bytes) in entries {
        frame(&mut stored, key, bytes);
    }
    crate::canonical_digest::digest(&stored)
}

fn frame(target: &mut Vec<u8>, relative: &str, bytes: &[u8]) {
    target.extend_from_slice(relative.as_bytes());
    target.push(0);
    target.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    target.extend_from_slice(bytes);
}

/// The canonical path whose bytes could not be hashed.
#[derive(Debug, thiserror::Error)]
#[error("cannot hash {path}: {error}")]
pub struct UnitHashError {
    pub path: Utf8PathBuf,
    #[source]
    pub error: anyhow::Error,
}

impl UnitHashError {
    fn at(path: &Utf8Path, error: impl Into<anyhow::Error>) -> Self {
        Self {
            path: path.to_path_buf(),
            error: error.into(),
        }
    }
}

#[cfg(test)]
mod tests;
