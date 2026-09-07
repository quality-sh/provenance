//! Hash units: one per manifest scope and one global unit.
//!
//! A scope unit is the scope's directory. The global unit is every regular
//! file under `state/` outside `scopes/`. A unit digest frames each file's
//! relative path and complete bytes in sorted path order, so two shards
//! that share a basename cannot swap contents unnoticed.

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{ensure_supported_schema_version, Manifest, ScopeId};

/// One hash unit. `name` is the key of its digest row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unit {
    Global,
    Scope(ScopeId),
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
    ensure_supported_schema_version("manifest", manifest.schema_version)?;
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

/// The digest of one unit's canonical bytes under `state_dir`.
pub fn unit_digest(state_dir: &Utf8Path, unit: &Unit) -> Result<String, UnitHashError> {
    digest_with(state_dir, unit, |_, _| {})
}

pub(super) fn digest_with(
    state_dir: &Utf8Path,
    unit: &Unit,
    mut retain: impl FnMut(&Utf8Path, &[u8]),
) -> Result<String, UnitHashError> {
    let files = unit_files(state_dir, unit)?;
    crate::test_probes::at("unit_files_collected")
        .map_err(|error| UnitHashError::at(state_dir, error))?;
    let mut framed = Vec::new();
    for (relative, path) in &files {
        let bytes = std::fs::read(path).map_err(|error| UnitHashError::at(path, error))?;
        retain(path, &bytes);
        framed.extend_from_slice(relative.as_bytes());
        framed.push(0);
        framed.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        framed.extend_from_slice(&bytes);
    }
    let after = unit_files(state_dir, unit)?;
    if files != after {
        let changed = files
            .iter()
            .find(|entry| after.binary_search(entry).is_err())
            .or_else(|| {
                after
                    .iter()
                    .find(|entry| files.binary_search(entry).is_err())
            })
            .expect("different file lists have a changed entry");
        return Err(UnitHashError::at(
            &changed.1,
            anyhow::anyhow!("canonical file list changed during hashing"),
        ));
    }
    Ok(crate::canonical_digest::digest(&framed))
}

fn unit_files(
    state_dir: &Utf8Path,
    unit: &Unit,
) -> Result<Vec<(String, Utf8PathBuf)>, UnitHashError> {
    let mut files = Vec::new();
    match unit {
        Unit::Global => collect(state_dir, state_dir, true, &mut files)?,
        Unit::Scope(scope) => {
            let root = state_dir.join("scopes").join(scope.as_str());
            match std::fs::symlink_metadata(&root) {
                Ok(_) => collect(&root, state_dir, false, &mut files)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(UnitHashError::at(&root, error)),
            }
        }
    }
    files.sort();
    Ok(files)
}

/// An atomic write stages a `.tmp*` file beside the shard. A crash can
/// leave it behind. No reader reads it.
fn is_write_residue(name: &str) -> bool {
    name.starts_with(".tmp")
}

fn collect(
    dir: &Utf8Path,
    base: &Utf8Path,
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
        let file_type = entry
            .file_type()
            .map_err(|error| UnitHashError::at(&path, error))?;
        if file_type.is_dir() {
            if skip_scopes && path == base.join("scopes") {
                continue;
            }
            collect(&path, base, skip_scopes, out)?;
        } else if file_type.is_file() && !is_write_residue(name) {
            let relative = path
                .strip_prefix(base)
                .map_err(|_| UnitHashError::at(&path, anyhow::anyhow!("{path} is outside {base}")))?
                .components()
                .map(|component| component.as_str())
                .collect::<Vec<_>>()
                .join("/");
            out.push((relative, path));
        } else if !file_type.is_file() {
            return Err(UnitHashError::at(
                &path,
                anyhow::anyhow!("unsupported state entry: {path}"),
            ));
        }
    }
    Ok(())
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
