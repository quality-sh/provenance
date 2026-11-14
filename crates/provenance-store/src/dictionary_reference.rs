//! The project dictionary that statement checks use. A repository identity
//! carries it; with the `host` feature, the committed reference resolves it
//! from the machine index directory.

#[cfg(feature = "host")]
use std::path::PathBuf;

#[cfg(feature = "host")]
use anyhow::Context;
#[cfg(feature = "host")]
use camino::Utf8PathBuf;
#[cfg(feature = "host")]
use provenance_core::protocol::read_failure::UnparsableStateFile;
#[cfg(feature = "host")]
use provenance_macros::rule;
use provenance_ste100::DictionaryImport;
#[cfg(feature = "host")]
use provenance_ste100::DictionaryImportIdentity;
#[cfg(feature = "host")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "host")]
use crate::layout::ProvenanceLayout;

#[cfg(feature = "host")]
pub fn dictionary_reference_path(layout: &ProvenanceLayout) -> Utf8PathBuf {
    layout.state_dir().join("dictionary.json")
}

/// The committed project dictionary reference: the identity of the machine
/// index, and the technical names that ASD-STE100 Rule 1.1 accepts.
#[derive(Debug, Serialize, Deserialize)]
#[cfg(feature = "host")]
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
#[cfg(feature = "host")]
pub enum DictionaryResolution {
    /// The project commits no dictionary reference.
    NoReference,
    /// The reference loaded its index from the machine directory.
    Loaded(ProjectDictionary),
    /// The reference does not parse, so it names no index. The refusal gives
    /// the file, the line, the parse message and the repair.
    Unparsable(UnparsableStateFile),
    /// The reference names an index that this machine cannot load.
    Unavailable {
        identity: DictionaryImportIdentity,
        directory: Option<PathBuf>,
        reason: String,
    },
}

/// The step that repairs a dictionary reference that does not parse.
#[cfg(feature = "host")]
const REFERENCE_REPAIR: &str =
    "get the file again from git or correct its JSON, and then do the command again";

/// Parses the reference at `path`. The refusal of a reference that does not
/// parse gives the file below the state directory, the line, the parse message
/// and the repair.
#[cfg(feature = "host")]
fn parse_reference(
    path: &camino::Utf8Path,
    bytes: &[u8],
) -> Result<DictionaryReference, UnparsableStateFile> {
    serde_json::from_slice(bytes).map_err(|error| {
        crate::state_store::readers::unparsable(
            path,
            u64::try_from(error.line()).unwrap_or(u64::MAX),
            crate::state_store::readers::parse_message(&error),
            Some(REFERENCE_REPAIR.to_owned()),
        )
    })
}

/// Separates a missing reference from a reference that cannot load, so a strict
/// gate can fail on the second and stay silent on the first.
#[cfg(feature = "host")]
pub fn resolve_project_dictionary(layout: &ProvenanceLayout) -> DictionaryResolution {
    let path = dictionary_reference_path(layout);
    let Ok(bytes) = std::fs::read(path.as_std_path()) else {
        return DictionaryResolution::NoReference;
    };
    let reference = match parse_reference(&path, &bytes) {
        Ok(reference) => reference,
        Err(refusal) => return DictionaryResolution::Unparsable(refusal),
    };
    let Some(directory) = index_directory() else {
        return DictionaryResolution::Unavailable {
            identity: reference.identity,
            directory: None,
            reason: "no machine data directory is available".to_owned(),
        };
    };
    match crate::dictionary_index::load_dictionary_index(&directory, &reference.identity) {
        Ok(import) => DictionaryResolution::Loaded(ProjectDictionary {
            import,
            technical_names: reference.technical_names,
        }),
        Err(error) => DictionaryResolution::Unavailable {
            identity: reference.identity,
            directory: Some(directory),
            reason: error.to_string(),
        },
    }
}

/// Loads the referenced dictionary, or nothing when it cannot load.
#[rule("rule_ste_dictionary_reference_resolution")]
#[cfg(feature = "host")]
pub fn load_project_dictionary(layout: &ProvenanceLayout) -> Option<ProjectDictionary> {
    match resolve_project_dictionary(layout) {
        DictionaryResolution::Loaded(dictionary) => Some(dictionary),
        DictionaryResolution::NoReference
        | DictionaryResolution::Unparsable(_)
        | DictionaryResolution::Unavailable { .. } => None,
    }
}

/// Stores the index in `index_directory` and then writes the project
/// reference, so that the reference names only an index that is stored.
///
/// A current reference that does not parse stops the call before a write.
#[cfg(feature = "host")]
pub fn set_project_dictionary(
    layout: &ProvenanceLayout,
    import: &DictionaryImport,
    index_directory: &std::path::Path,
) -> anyhow::Result<Utf8PathBuf> {
    let path = dictionary_reference_path(layout);
    let reference = current_reference(layout)?.bytes_for(&import.identity)?;
    crate::dictionary_index::store_dictionary_index(import, index_directory)
        .context("store the dictionary index")?;
    crate::storage::local::durability::write_synced_file(&path, &reference)
        .context("write the dictionary reference")?;
    Ok(path)
}

/// The part of the committed reference that a write of a new dictionary
/// identity keeps: the technical names.
#[cfg(feature = "host")]
#[derive(Debug)]
pub struct CurrentReference {
    technical_names: Vec<String>,
}

#[cfg(feature = "host")]
impl CurrentReference {
    /// The reference bytes for `identity`, with the technical names of the
    /// current reference.
    pub fn bytes_for(&self, identity: &DictionaryImportIdentity) -> anyhow::Result<Vec<u8>> {
        let reference = DictionaryReference {
            identity: identity.clone(),
            technical_names: self.technical_names.clone(),
        };
        let mut bytes =
            serde_json::to_vec_pretty(&reference).context("serialize the dictionary reference")?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}

/// Reads the committed reference and gives its part that a write keeps, or the
/// refusal of [`current_reference_from`].
#[cfg(feature = "host")]
pub fn current_reference(layout: &ProvenanceLayout) -> anyhow::Result<CurrentReference> {
    let path = dictionary_reference_path(layout);
    let bytes = match std::fs::read(path.as_std_path()) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error).context("read the dictionary reference"),
    };
    Ok(current_reference_from(layout, bytes.as_deref())?)
}

/// Refuses a dictionary reference that does not parse, so that no write of
/// the reference loses its technical names.
#[rule("rule_ste_dictionary_reference_parse_refusal")]
#[cfg(feature = "host")]
pub fn current_reference_from(
    layout: &ProvenanceLayout,
    file: Option<&[u8]>,
) -> Result<CurrentReference, UnparsableStateFile> {
    let technical_names = match file {
        None => Vec::new(),
        Some(bytes) => parse_reference(&dictionary_reference_path(layout), bytes)?.technical_names,
    };
    Ok(CurrentReference { technical_names })
}

/// The warning for a reference that exists without a loadable index, because
/// then the statement checks do not run ASD-STE100 Rule 1.1.
#[rule("rule_ste_dictionary_index_missing_warning")]
#[cfg(feature = "host")]
pub fn missing_index_warning(layout: &ProvenanceLayout) -> Option<String> {
    match resolve_project_dictionary(layout) {
        DictionaryResolution::NoReference | DictionaryResolution::Loaded(_) => None,
        DictionaryResolution::Unparsable(refusal) => {
            Some(format!("ASD-STE100 Rule 1.1 did not run: {refusal}"))
        }
        DictionaryResolution::Unavailable {
            directory, reason, ..
        } => {
            let directory = directory.map_or_else(
                || "the machine data directory".to_owned(),
                |directory| directory.display().to_string(),
            );
            Some(format!(
                "ASD-STE100 Rule 1.1 did not run: the dictionary reference has no loadable \
                 index in {directory}: {reason}. Run `provenance dictionary import` with the \
                 local Issue 9 PDF on this machine"
            ))
        }
    }
}

/// The machine directory that holds imported dictionary index files.
#[cfg(feature = "host")]
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
#[cfg(feature = "host")]
fn data_directory() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

#[cfg(target_os = "macos")]
#[cfg(feature = "host")]
fn data_directory() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
}

#[cfg(all(unix, not(target_os = "macos")))]
#[cfg(feature = "host")]
fn data_directory() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
}

#[cfg(test)]
#[cfg(all(unix, feature = "host"))]
mod write_tests;
