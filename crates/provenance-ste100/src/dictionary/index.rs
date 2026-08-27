use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
};

use fs2::FileExt;
use provenance_macros::rule;

use super::{digest, DictionaryImport, DictionaryImportIdentity};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DictionaryIndexError {
    Io { path: PathBuf, message: String },
    NotFound { path: PathBuf },
    Malformed { path: PathBuf, message: String },
    IdentityMismatch { path: PathBuf },
    DigestMismatch { path: PathBuf },
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
        }
    }
}

impl std::error::Error for DictionaryIndexError {}

/// Stores one import as a reusable index file named by its identity.
#[rule("rule_ste_dictionary_import_reuse")]
pub fn store_dictionary_index(
    import: &DictionaryImport,
    directory: &Path,
) -> Result<PathBuf, DictionaryIndexError> {
    let path = index_path(directory, &import.identity);
    std::fs::create_dir_all(directory).map_err(|error| io_error(directory, &error))?;
    let lock_path = path.with_extension("lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| io_error(&lock_path, &error))?;
    FileExt::lock_exclusive(&lock).map_err(|error| io_error(&lock_path, &error))?;
    match load_dictionary_index(directory, &import.identity) {
        Ok(stored) if stored == *import => return Ok(path),
        Ok(_) => std::fs::remove_file(&path).map_err(|error| io_error(&path, &error))?,
        Err(DictionaryIndexError::NotFound { .. }) => {}
        Err(_) => std::fs::remove_file(&path).map_err(|error| io_error(&path, &error))?,
    }
    let contents = serde_json::to_vec(import).map_err(|error| DictionaryIndexError::Io {
        path: path.clone(),
        message: error.to_string(),
    })?;
    let staged = path.with_extension("partial");
    std::fs::write(&staged, contents).map_err(|error| io_error(&staged, &error))?;
    std::fs::rename(&staged, &path).map_err(|error| io_error(&path, &error))?;
    Ok(path)
}

/// Loads a stored index only when it matches the requested identity.
pub fn load_dictionary_index(
    directory: &Path,
    identity: &DictionaryImportIdentity,
) -> Result<DictionaryImport, DictionaryIndexError> {
    let path = index_path(directory, identity);
    let contents = match std::fs::read(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(DictionaryIndexError::NotFound { path });
        }
        Err(error) => return Err(io_error(&path, &error)),
    };
    let import: DictionaryImport =
        serde_json::from_slice(&contents).map_err(|error| DictionaryIndexError::Malformed {
            path: path.clone(),
            message: error.to_string(),
        })?;
    verify_index(&import, identity, &path)?;
    Ok(import)
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

fn index_path(directory: &Path, identity: &DictionaryImportIdentity) -> PathBuf {
    directory.join(format!(
        "issue-{}-{}-{}.json",
        u8::from(identity.issue),
        identity.source_sha256,
        identity.extractor_version
    ))
}

fn io_error(path: &Path, error: &std::io::Error) -> DictionaryIndexError {
    DictionaryIndexError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use provenance_macros::verifies;

    use super::super::{digest, DictionaryEntry, DictionaryStatus, PartOfSpeech};
    use super::{load_dictionary_index, store_dictionary_index, DictionaryIndexError};

    static DIRECTORY_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn scratch_directory() -> PathBuf {
        let ordinal = DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "provenance-ste100-index-{}-{ordinal}",
            std::process::id()
        ))
    }

    fn fixture_import() -> super::DictionaryImport {
        let entries = vec![
            DictionaryEntry {
                headword: "AAWORD".to_owned(),
                word_forms: vec!["AAWORD".to_owned()],
                part_of_speech: PartOfSpeech::Noun,
                status: DictionaryStatus::Approved,
                approved_meaning_or_alternatives: "A meaning".to_owned(),
                ste_example: "USE THE ITEM.".to_owned(),
                non_ste_example: None,
            },
            DictionaryEntry {
                headword: "bword".to_owned(),
                word_forms: vec!["bword".to_owned()],
                part_of_speech: PartOfSpeech::Verb,
                status: DictionaryStatus::Unapproved,
                approved_meaning_or_alternatives: "USE (v)".to_owned(),
                ste_example: "USE THE ITEM.".to_owned(),
                non_ste_example: Some("Bword the item.".to_owned()),
            },
        ];
        super::DictionaryImport {
            identity: digest::identity(b"synthetic source bytes", &entries),
            entries,
        }
    }

    #[test]
    #[verifies("rule_ste_dictionary_import_reuse", examples)]
    fn a_stored_index_loads_again_without_the_source_document() {
        let directory = scratch_directory();
        let import = fixture_import();

        let path = store_dictionary_index(&import, &directory).expect("store the index");
        let loaded = load_dictionary_index(&directory, &import.identity).expect("load the index");

        assert!(path.starts_with(&directory));
        assert_eq!(loaded, import);
        std::fs::remove_dir_all(&directory).expect("remove the scratch directory");
    }

    #[test]
    #[verifies("rule_ste_dictionary_import_reuse", examples)]
    fn storing_an_identical_index_again_is_idempotent() {
        let directory = scratch_directory();
        let import = fixture_import();

        let first = store_dictionary_index(&import, &directory).expect("store the first index");
        let second = store_dictionary_index(&import, &directory).expect("store the index again");

        assert_eq!(second, first);
        assert_eq!(
            load_dictionary_index(&directory, &import.identity).expect("load the index"),
            import
        );
        std::fs::remove_dir_all(&directory).expect("remove the scratch directory");
    }

    #[test]
    #[verifies("rule_ste_dictionary_index_digest_verification", examples)]
    fn an_edited_index_file_fails_closed() {
        let directory = scratch_directory();
        let import = fixture_import();

        let path = store_dictionary_index(&import, &directory).expect("store the index");
        let stored = std::fs::read_to_string(&path).expect("read the index file");
        let edited = stored.replace("AAWORD", "ABWORD");
        assert_ne!(stored, edited, "the edit must change the file");
        std::fs::write(&path, edited).expect("edit the index file");

        let error = load_dictionary_index(&directory, &import.identity)
            .expect_err("an edited index must fail closed");

        assert!(
            matches!(error, DictionaryIndexError::DigestMismatch { .. }),
            "unexpected error: {error:?}"
        );
        std::fs::remove_dir_all(&directory).expect("remove the scratch directory");
    }

    #[test]
    #[verifies("rule_ste_dictionary_index_digest_verification", examples)]
    fn an_index_for_a_different_identity_fails_closed() {
        let directory = scratch_directory();
        let import = fixture_import();

        store_dictionary_index(&import, &directory).expect("store the index");
        let mut other = import.identity;
        other.extractor_version = format!("{}-other", other.extractor_version);

        let error = load_dictionary_index(&directory, &other)
            .expect_err("a different identity must not load this index");

        assert!(
            matches!(error, DictionaryIndexError::NotFound { .. }),
            "unexpected error: {error:?}"
        );
        std::fs::remove_dir_all(&directory).expect("remove the scratch directory");
    }

    #[test]
    #[verifies("rule_ste_dictionary_index_digest_verification", examples)]
    fn an_index_file_with_a_different_recorded_identity_fails_closed() {
        let directory = scratch_directory();
        let import = fixture_import();
        let mut other = import.identity.clone();
        other.extractor_version = format!("{}-other", other.extractor_version);

        let path = store_dictionary_index(&import, &directory).expect("store the index");
        let other_path = super::index_path(&directory, &other);
        std::fs::rename(&path, &other_path).expect("move the index file");

        let error = load_dictionary_index(&directory, &other)
            .expect_err("a moved index must not satisfy another identity");

        assert!(
            matches!(error, DictionaryIndexError::IdentityMismatch { .. }),
            "unexpected error: {error:?}"
        );
        std::fs::remove_dir_all(&directory).expect("remove the scratch directory");
    }

    #[test]
    fn a_missing_index_reports_not_found() {
        let directory = scratch_directory();
        let import = fixture_import();

        let error =
            load_dictionary_index(&directory, &import.identity).expect_err("nothing was stored");

        assert!(
            matches!(error, DictionaryIndexError::NotFound { .. }),
            "unexpected error: {error:?}"
        );
    }
}
