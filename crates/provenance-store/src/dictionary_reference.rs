use std::path::PathBuf;

use anyhow::Context;
use camino::Utf8PathBuf;
use provenance_macros::rule;
use provenance_ste100::{DictionaryImport, DictionaryImportIdentity};

use crate::layout::ProvenanceLayout;

pub fn dictionary_reference_path(layout: &ProvenanceLayout) -> Utf8PathBuf {
    layout.state_dir().join("dictionary.json")
}

/// How the project dictionary reference resolved on this machine.
#[derive(Debug)]
pub enum DictionaryResolution {
    /// The project commits no dictionary reference.
    NoReference,
    /// The reference loaded its index from the machine directory.
    Loaded(DictionaryImport),
    /// The reference exists, but this machine holds no matching index for it.
    Unavailable {
        identity: Option<DictionaryImportIdentity>,
        directory: Option<PathBuf>,
        reason: String,
    },
}

/// Separates a missing reference from a reference that cannot load, so a strict
/// gate can fail on the second and stay silent on the first.
pub fn resolve_project_dictionary(layout: &ProvenanceLayout) -> DictionaryResolution {
    let Ok(reference) = std::fs::read(dictionary_reference_path(layout).as_std_path()) else {
        return DictionaryResolution::NoReference;
    };
    let identity: DictionaryImportIdentity = match serde_json::from_slice(&reference) {
        Ok(identity) => identity,
        Err(error) => {
            return DictionaryResolution::Unavailable {
                identity: None,
                directory: index_directory(),
                reason: format!("the reference is not a dictionary identity: {error}"),
            };
        }
    };
    let Some(directory) = index_directory() else {
        return DictionaryResolution::Unavailable {
            identity: Some(identity),
            directory: None,
            reason: "no machine data directory is available".to_owned(),
        };
    };
    match provenance_ste100::load_dictionary_index(&directory, &identity) {
        Ok(dictionary) => DictionaryResolution::Loaded(dictionary),
        Err(error) => DictionaryResolution::Unavailable {
            identity: Some(identity),
            directory: Some(directory),
            reason: error.to_string(),
        },
    }
}

/// Loads the referenced dictionary, or nothing when it cannot load.
#[rule("rule_ste_dictionary_reference_resolution")]
pub fn load_project_dictionary(layout: &ProvenanceLayout) -> Option<DictionaryImport> {
    match resolve_project_dictionary(layout) {
        DictionaryResolution::Loaded(dictionary) => Some(dictionary),
        DictionaryResolution::NoReference | DictionaryResolution::Unavailable { .. } => None,
    }
}

/// Stores the index in the machine data directory and writes the project reference.
pub fn set_project_dictionary(
    layout: &ProvenanceLayout,
    import: &DictionaryImport,
) -> anyhow::Result<Utf8PathBuf> {
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
