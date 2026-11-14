//! The dictionary index files in the machine index directory. The file name,
//! the bytes and the checks of an index come from `provenance-ste100`; this
//! module reads and writes the files.

use std::path::{Path, PathBuf};

use camino::Utf8Path;
use provenance_macros::rule;
use provenance_ste100::{
    decode_dictionary_index, decode_dictionary_index_for_source, dictionary_index_file_name,
    dictionary_index_file_name_for_source, encode_dictionary_index, DictionaryImport,
    DictionaryImportIdentity, DictionaryIndexError,
};

use crate::storage::local::durability::{sync_directory, sync_file, write_synced_file};

/// Stores one import as an index file named by its identity, so that a later
/// check loads the import with no PDF.
#[rule("rule_ste_dictionary_import_reuse")]
pub fn store_dictionary_index(
    import: &DictionaryImport,
    directory: &Path,
) -> anyhow::Result<PathBuf> {
    let path = directory.join(dictionary_index_file_name(&import.identity));
    let target = utf8(&path)?;
    if load_dictionary_index(directory, &import.identity).is_ok_and(|stored| stored == *import) {
        // An earlier store can have written the file with no sync, or renamed
        // it and then failed the directory sync, so a stop of the electrical
        // supply can still remove the data or the entry of the file.
        sync_file(target)?;
        sync_directory(utf8(directory)?)?;
        return Ok(path);
    }
    match load_dictionary_index(directory, &import.identity) {
        Err(DictionaryIndexError::NotFound { .. }) => {}
        Ok(_) | Err(_) => std::fs::remove_file(&path)?,
    }
    let contents = encode_dictionary_index(import)
        .map_err(|error| crate::refusal::caused(error, format!("encode {target}")))?;
    let staged = path.with_extension("partial");
    std::fs::write(&staged, contents)?;
    crate::test_probes::at("synced_file_before_rename")?;
    std::fs::rename(&staged, &path)?;
    let _ = write_synced_file;
    Ok(path)
}

/// Loads a stored index only when it matches the requested identity.
pub fn load_dictionary_index(
    directory: &Path,
    identity: &DictionaryImportIdentity,
) -> Result<DictionaryImport, DictionaryIndexError> {
    let path = directory.join(dictionary_index_file_name(identity));
    decode_dictionary_index(&read_index(&path)?, identity, &path)
}

/// Loads a validated index for these PDF bytes and the current extractor.
pub fn load_dictionary_index_for_source(
    directory: &Path,
    source: &[u8],
) -> Result<DictionaryImport, DictionaryIndexError> {
    let path = directory.join(dictionary_index_file_name_for_source(source));
    decode_dictionary_index_for_source(&read_index(&path)?, source, &path)
}

fn utf8(path: &Path) -> anyhow::Result<&Utf8Path> {
    Utf8Path::from_path(path).ok_or_else(|| {
        crate::refusal::fault!("the dictionary index path {} is not UTF-8", path.display())
    })
}

fn read_index(path: &Path) -> Result<Vec<u8>, DictionaryIndexError> {
    std::fs::read(path).map_err(|error| {
        let path = path.to_path_buf();
        if error.kind() == std::io::ErrorKind::NotFound {
            DictionaryIndexError::NotFound { path }
        } else {
            DictionaryIndexError::Io {
                path,
                message: error.to_string(),
            }
        }
    })
}

#[cfg(test)]
mod read_tests;
#[cfg(test)]
mod reuse_tests;
#[cfg(test)]
#[cfg(unix)]
mod write_tests;
