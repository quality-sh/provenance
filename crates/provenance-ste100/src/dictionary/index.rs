//! The format of a dictionary index file: its name, its bytes and the checks
//! of an index that a reader gives back. The file I/O is in `provenance-store`.

use std::path::{Path, PathBuf};

use provenance_macros::rule;

use super::{digest, DictionaryImport, DictionaryImportIdentity, DICTIONARY_EXTRACTOR_VERSION};
use crate::StandardIssue;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DictionaryIndexError {
    Io { path: PathBuf, message: String },
    NotFound { path: PathBuf },
    Malformed { path: PathBuf, message: String },
    IdentityMismatch { path: PathBuf },
    DigestMismatch { path: PathBuf },
    InvalidStructure { path: PathBuf, message: String },
}

impl std::fmt::Display for DictionaryIndexError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } => {
                write!(
                    formatter,
                    "index file access at {}: {message}",
                    path.display()
                )
            }
            Self::NotFound { path } => {
                write!(formatter, "no index file at {}", path.display())
            }
            Self::Malformed { path, message } => {
                write!(
                    formatter,
                    "malformed index file at {}: {message}",
                    path.display()
                )
            }
            Self::IdentityMismatch { path } => write!(
                formatter,
                "the index file at {} records a different import identity",
                path.display()
            ),
            Self::DigestMismatch { path } => write!(
                formatter,
                "the index file at {} does not match its recorded data digest",
                path.display()
            ),
            Self::InvalidStructure { path, message } => write!(
                formatter,
                "the index file at {} has invalid dictionary structure: {message}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for DictionaryIndexError {}

/// The name of the index file of `identity` in the index directory.
pub fn dictionary_index_file_name(identity: &DictionaryImportIdentity) -> String {
    file_name(
        identity.issue,
        &identity.source_sha256,
        &identity.extractor_version,
    )
}

/// The name of the index file of an import of the PDF `source` with this
/// extractor.
pub fn dictionary_index_file_name_for_source(source: &[u8]) -> String {
    file_name(
        StandardIssue::Nine,
        &digest::source_digest(source),
        DICTIONARY_EXTRACTOR_VERSION,
    )
}

/// The bytes of the index file of `import`.
pub fn encode_dictionary_index(import: &DictionaryImport) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec(import)
}

/// The import in the index file `contents` at `path`, only when it matches
/// the requested identity.
pub fn decode_dictionary_index(
    contents: &[u8],
    requested: &DictionaryImportIdentity,
    path: &Path,
) -> Result<DictionaryImport, DictionaryIndexError> {
    let import = parse_index(contents, path)?;
    verify_index(&import, requested, path)?;
    Ok(import)
}

/// The import in the index file `contents` at `path`, only when it is a
/// complete import of the PDF `source` with this extractor.
pub fn decode_dictionary_index_for_source(
    contents: &[u8],
    source: &[u8],
    path: &Path,
) -> Result<DictionaryImport, DictionaryIndexError> {
    let import = parse_index(contents, path)?;
    let requested = DictionaryImportIdentity {
        issue: StandardIssue::Nine,
        source_sha256: digest::source_digest(source),
        data_sha256: import.identity.data_sha256.clone(),
        extractor_version: DICTIONARY_EXTRACTOR_VERSION.to_owned(),
    };
    verify_index(&import, &requested, path)?;
    super::validate_entries(&import.entries).map_err(|error| {
        DictionaryIndexError::InvalidStructure {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    Ok(import)
}

fn parse_index(contents: &[u8], path: &Path) -> Result<DictionaryImport, DictionaryIndexError> {
    serde_json::from_slice(contents).map_err(|error| DictionaryIndexError::Malformed {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

/// Rejects a stored index whose content does not match its recorded digest.
#[rule("rule_ste_dictionary_index_digest_verification")]
fn verify_index(
    import: &DictionaryImport,
    requested: &DictionaryImportIdentity,
    path: &Path,
) -> Result<(), DictionaryIndexError> {
    if &import.identity != requested {
        return Err(DictionaryIndexError::IdentityMismatch {
            path: path.to_path_buf(),
        });
    }
    if digest::normalized_data_digest(&import.entries) != import.identity.data_sha256 {
        return Err(DictionaryIndexError::DigestMismatch {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn file_name(issue: StandardIssue, source_sha256: &str, extractor_version: &str) -> String {
    format!(
        "issue-{}-{}-{}.json",
        u8::from(issue),
        source_sha256,
        extractor_version
    )
}

#[cfg(test)]
mod tests;
