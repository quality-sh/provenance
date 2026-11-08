//! Hash units: one per manifest scope and one global unit.
//!
//! The units cover the canonical state of one view: the manifest and each
//! shard of a scope. A scope unit is the keys below the scope's directory.
//! The global unit is every other key. Any other entry under `state/`, such
//! as a temporary file of a write, is not in the view and moves no digest.
//! A unit digest frames each key and its content bytes in key order, so two
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

/// The digest of one unit's canonical bytes under `state_dir`, read with no
/// lock.
pub fn unit_digest(state_dir: &Utf8Path, unit: &Unit) -> Result<String, UnitHashError> {
    Ok(unit_digests(state_dir, &state_view(state_dir)?, unit, None)?.content)
}

/// The digest of the exact stored bytes in one unit, read with no lock.
pub fn unit_stored_digest(state_dir: &Utf8Path, unit: &Unit) -> Result<String, UnitHashError> {
    Ok(stored_digest(&unit_entries(&state_view(state_dir)?, unit)))
}

/// The canonical state under `state_dir`.
fn state_view(state_dir: &Utf8Path) -> Result<StateView, UnitHashError> {
    crate::storage::local::read_state_tree(state_dir)
        .map_err(|error| UnitHashError::at(state_dir, error))
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
    let mut content = Vec::new();
    for (key, bytes) in &entries {
        let path = state_dir.join(key);
        let content_bytes =
            content::hash_bytes(&path, bytes).map_err(|error| UnitHashError::at(&path, error))?;
        frame(&mut content, key, &content_bytes);
    }
    Ok(UnitDigests {
        content: crate::canonical_digest::digest(&content),
        stored,
    })
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
