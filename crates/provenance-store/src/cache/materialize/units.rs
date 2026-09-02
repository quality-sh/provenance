//! Hash units: one per manifest scope and one global unit.
//!
//! A scope unit is the scope's directory. The global unit is every regular
//! file under `state/` outside `scopes/`. A unit digest frames each file's
//! relative path and complete bytes in sorted path order, so two shards
//! that share a basename cannot swap contents unnoticed.

use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::ScopeId;

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

/// Every unit a manifest names, global first, scopes sorted.
pub fn units_for(scopes: &[ScopeId]) -> Vec<Unit> {
    let mut sorted: Vec<&ScopeId> = scopes.iter().collect();
    sorted.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    let mut units = vec![Unit::Global];
    units.extend(sorted.into_iter().map(|scope| Unit::Scope(scope.clone())));
    units
}

/// The digest of one unit's canonical bytes under `state_dir`.
pub fn unit_digest(state_dir: &Utf8Path, unit: &Unit) -> anyhow::Result<String> {
    let mut files = Vec::new();
    match unit {
        Unit::Global => collect(state_dir, state_dir, true, &mut files)?,
        Unit::Scope(scope) => {
            let root = state_dir.join("scopes").join(scope.as_str());
            if root.is_dir() {
                collect(&root, state_dir, false, &mut files)?;
            }
        }
    }
    files.sort();
    let mut framed = Vec::new();
    for (relative, path) in files {
        let bytes = std::fs::read(&path)?;
        framed.extend_from_slice(relative.as_bytes());
        framed.push(0);
        framed.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        framed.extend_from_slice(&bytes);
    }
    Ok(crate::canonical_digest::digest(&framed))
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
) -> anyhow::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = Utf8PathBuf::from_path_buf(entry.path())
            .map_err(|path| anyhow::anyhow!("non-UTF-8 canonical path: {}", path.display()))?;
        let name = path.file_name().unwrap_or_default();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if skip_scopes && path == base.join("scopes") {
                continue;
            }
            collect(&path, base, skip_scopes, out)?;
        } else if file_type.is_file() && !is_write_residue(name) {
            let relative = path
                .strip_prefix(base)
                .map_err(|_| anyhow::anyhow!("{path} is outside {base}"))?
                .components()
                .map(|component| component.as_str())
                .collect::<Vec<_>>()
                .join("/");
            out.push((relative, path));
        }
    }
    Ok(())
}
