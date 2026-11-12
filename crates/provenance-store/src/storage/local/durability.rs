use camino::{Utf8Path, Utf8PathBuf};

/// A failure of [`replace_file`]. It has the text of a `tempfile` persist
/// failure, and its source is the I/O error.
#[derive(Debug, thiserror::Error)]
#[error("failed to persist temporary file: {0}")]
pub struct ReplaceFailed(#[source] std::io::Error);

/// Replaces the file at `target` with `temporary`, a file in the same
/// directory. The target has all of the new bytes or keeps its old bytes. A
/// handle that a reader has open on the target does not make the replace fail
/// when that handle lets other handles delete the file, as `std::fs` handles
/// do. The temporary file stays open until the rename returns. After a
/// failure, the temporary file is deleted.
///
/// This function does not sync. The writers of the publication marker and of
/// the Source edit marker sync the temporary file before the call and the
/// directory after it. `write_file_atomic` syncs neither: a whole-set
/// publication syncs the staged tree after all of its writes, and a one-key
/// write and a JSONL file write do not sync.
pub fn replace_file(temporary: tempfile::NamedTempFile, target: &Utf8Path) -> anyhow::Result<()> {
    let target = crate::long_path::extended(target)?;
    // On Windows, `keep` clears the temporary attribute that `tempfile` sets.
    // A rename keeps the attributes of the file, and with that attribute the
    // system can keep the bytes in memory and not write them to the disk.
    let (file, path) = temporary
        .keep()
        .map_err(|failure| ReplaceFailed(failure.error))?;
    let renamed = std::fs::rename(&path, target);
    drop(file);
    renamed.map_err(|error| {
        let _ = std::fs::remove_file(&path);
        ReplaceFailed(error)
    })?;
    Ok(())
}

/// Flushes directory metadata where the platform supports it.
///
/// Windows has no equivalent call in this module. On Windows, file contents are flushed, but a
/// sudden power loss can lose a recent directory entry or rename. Process-crash recovery remains
/// available because the operating system keeps the directory changes.
pub fn sync_directory(path: &Utf8Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    std::fs::File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

pub fn sync_tree(path: &Utf8Path) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(path)
        .map_err(|error| crate::refusal::fault!("list publication tree {path}: {error}"))?
    {
        let entry = entry?;
        let child = Utf8PathBuf::from_path_buf(entry.path()).map_err(|path| {
            crate::refusal::fault!("publication path is not UTF-8: {}", path.display())
        })?;
        if entry.file_type()?.is_dir() {
            sync_tree(&child)?;
        } else {
            // Windows' FlushFileBuffers needs write access; a read-only
            // handle is refused with ERROR_ACCESS_DENIED. Unix fsyncs a
            // read descriptor happily.
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(cfg!(windows))
                .open(&child)
                .map_err(|error| {
                    crate::refusal::fault!("sync publication file {child}: {error}")
                })?;
            file.sync_all().map_err(|error| {
                crate::refusal::fault!("sync publication file {child}: {error}")
            })?;
        }
    }
    sync_directory(path)
}
