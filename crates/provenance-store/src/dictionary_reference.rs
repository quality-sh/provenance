use std::path::PathBuf;

use anyhow::Context;
use camino::Utf8PathBuf;
use provenance_macros::rule;
use provenance_ste100::{DictionaryImport, DictionaryImportIdentity};
use serde::{Deserialize, Serialize};

use crate::layout::ProvenanceLayout;

pub fn dictionary_reference_path(layout: &ProvenanceLayout) -> Utf8PathBuf {
    layout.state_dir().join("dictionary.json")
}

/// The committed project dictionary reference: the identity of the machine
/// index, and the technical names that ASD-STE100 Rule 1.1 accepts.
#[derive(Debug, Serialize, Deserialize)]
struct DictionaryReference {
    #[serde(flatten)]
    identity: DictionaryImportIdentity,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    technical_names: Vec<String>,
}

/// The imported dictionary and the technical names of the project.
#[derive(Clone, Debug)]
pub struct ProjectDictionary {
    pub import: DictionaryImport,
    /// Technical nouns (ASD-STE100 Rule 1.5) and technical verbs (Rule 1.12)
    /// of the project, which Rule 1.1 accepts.
    pub technical_names: Vec<String>,
}

/// How the project dictionary reference resolved on this machine.
#[derive(Debug)]
pub enum DictionaryResolution {
    /// The project commits no dictionary reference.
    NoReference,
    /// The reference loaded its index from the machine directory.
    Loaded(ProjectDictionary),
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
    let reference: DictionaryReference = match serde_json::from_slice(&reference) {
        Ok(reference) => reference,
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
            identity: Some(reference.identity),
            directory: None,
            reason: "no machine data directory is available".to_owned(),
        };
    };
    match provenance_ste100::load_dictionary_index(&directory, &reference.identity) {
        Ok(import) => DictionaryResolution::Loaded(ProjectDictionary {
            import,
            technical_names: reference.technical_names,
        }),
        Err(error) => DictionaryResolution::Unavailable {
            identity: Some(reference.identity),
            directory: Some(directory),
            reason: error.to_string(),
        },
    }
}

/// Loads the referenced dictionary, or nothing when it cannot load.
#[rule("rule_ste_dictionary_reference_resolution")]
pub fn load_project_dictionary(layout: &ProvenanceLayout) -> Option<ProjectDictionary> {
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
    let reference = reference_bytes(layout, &import.identity)?;
    std::fs::write(path.as_std_path(), reference).context("write the dictionary reference")?;
    Ok(path)
}

/// The reference bytes for a new dictionary identity. The bytes keep the
/// technical names of the current reference.
pub fn reference_bytes(
    layout: &ProvenanceLayout,
    identity: &DictionaryImportIdentity,
) -> anyhow::Result<Vec<u8>> {
    let technical_names = match std::fs::read(dictionary_reference_path(layout).as_std_path()) {
        Ok(current) => serde_json::from_slice::<DictionaryReference>(&current)
            .map(|current| current.technical_names)
            .unwrap_or_default(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error).context("read the dictionary reference"),
    };
    let reference = DictionaryReference {
        identity: identity.clone(),
        technical_names,
    };
    let mut bytes =
        serde_json::to_vec_pretty(&reference).context("serialize the dictionary reference")?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// The warning for a reference that exists without a loadable index, because
/// then the statement checks do not run ASD-STE100 Rule 1.1.
#[rule("rule_ste_dictionary_index_missing_warning")]
pub fn missing_index_warning(layout: &ProvenanceLayout) -> Option<String> {
    let DictionaryResolution::Unavailable {
        directory, reason, ..
    } = resolve_project_dictionary(layout)
    else {
        return None;
    };
    let directory = directory.map_or_else(
        || "the machine data directory".to_owned(),
        |directory| directory.display().to_string(),
    );
    Some(format!(
        "ASD-STE100 Rule 1.1 did not run: the dictionary reference has no loadable index \
         in {directory}: {reason}. Run `provenance dictionary import` with the local Issue 9 \
         PDF on this machine"
    ))
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
