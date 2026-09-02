//! Hash units: one per manifest scope, one global.
//!
//! A unit digest covers the relative path and the complete bytes of every
//! regular canonical file in the unit, in sorted path order. The path is
//! framed, not the basename, so two shards that share a basename
//! (`implementations/binding.jsonl`, `verifications/binding.jsonl`) cannot
//! swap contents unnoticed. Temporary write residue — the `.tmp*` files an
//! atomic write leaves beside a shard when it dies mid-flight — is ignored,
//! because no reader reads it.
//!
//! The scope unit is the scope's directory. The global unit is every
//! regular canonical file under `state/` outside `scopes/`: the manifest,
//! the edge shards, the dictionary, and anything a later layout adds there.
//! A per-scope hash therefore covers every byte a reader can use without a
//! hand-written list of reader inputs.

use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::ScopeId;

/// One hash unit, named as its digest row stores it.
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

/// Atomic writes stage a `.tmp*` file beside the shard and rename it into
/// place; a crash can leave the stage behind. No reader reads it.
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
