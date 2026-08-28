use std::path::PathBuf;

use anyhow::Context;
use camino::Utf8PathBuf;
use provenance_macros::rule;
use provenance_ste100::{DictionaryImport, DictionaryImportIdentity};

use crate::layout::ProvenanceLayout;

pub fn dictionary_reference_path(layout: &ProvenanceLayout) -> Utf8PathBuf {
    layout.state_dir().join("dictionary.json")
}

/// Loads the referenced dictionary, or nothing when it cannot load.
#[rule("rule_ste_dictionary_reference_resolution")]
pub fn load_project_dictionary(layout: &ProvenanceLayout) -> Option<DictionaryImport> {
    let reference = std::fs::read(dictionary_reference_path(layout).as_std_path()).ok()?;
    let identity: DictionaryImportIdentity = serde_json::from_slice(&reference).ok()?;
    let directory = index_directory()?;
    provenance_ste100::load_dictionary_index(&directory, &identity).ok()
}

/// Stores the index in the machine data directory and writes the project reference.
///
/// This write bypasses both mutation primitives (a direct `fs::write` of
/// `state/dictionary.json`), so it carries its own named gate: on an
/// rbac-managed repository the claim must hold `edit` on every scope then
/// listed — the settled Option A rule for repo-global resources.
pub fn set_project_dictionary(
    layout: &ProvenanceLayout,
    claim: Option<&provenance_core::RbacClaim>,
    import: &DictionaryImport,
) -> anyhow::Result<Utf8PathBuf> {
    let manifest_path = layout.manifest_path();
    if manifest_path.exists() {
        let manifest: provenance_core::Manifest =
            serde_json::from_str(&std::fs::read_to_string(manifest_path)?)?;
        if let Some(section) = &manifest.rbac {
            let scopes: Vec<provenance_core::ScopeId> = manifest
                .scopes
                .iter()
                .map(|scope| scope.id.clone())
                .collect();
            provenance_core::authorize(
                claim,
                section,
                provenance_core::Capability::Edit,
                provenance_core::RbacResource::RepoGlobal(&scopes),
            )?;
        }
    }
    let directory = index_directory().context("no machine data directory is available")?;
    provenance_ste100::store_dictionary_index(import, &directory)
        .context("store the dictionary index")?;
    let path = dictionary_reference_path(layout);
    std::fs::create_dir_all(layout.state_dir().as_std_path())
        .context("create the project state directory")?;
    let mut reference = serde_json::to_vec_pretty(&import.identity)
        .context("serialize the dictionary reference")?;
    reference.push(b'\n');
    std::fs::write(path.as_std_path(), reference).context("write the dictionary reference")?;
    Ok(path)
}

/// The machine directory that holds imported dictionary index files.
pub fn index_directory() -> Option<PathBuf> {
    if let Some(directory) = std::env::var_os("PROVENANCE_STE100_INDEX_DIR") {
        return Some(PathBuf::from(directory));
    }
    Some(
        data_directory()?
            .join("provenance")
            .join("ste100-dictionary"),
    )
}

#[cfg(target_os = "windows")]
fn data_directory() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

#[cfg(target_os = "macos")]
fn data_directory() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn data_directory() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
}
