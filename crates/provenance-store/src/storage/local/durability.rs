use camino::{Utf8Path, Utf8PathBuf};
use std::io::Write;

/// A failure of [`replace_file`]. It has the text of a `tempfile` persist
/// failure, and its source is the I/O error.
#[derive(Debug, thiserror::Error)]
#[error("failed to persist temporary file: {0}")]
pub struct ReplaceFailed(#[source] std::io::Error);

/// Writes `bytes` to the file at `path` in this order: make the directory of
/// `path` if it does not exist, write a temporary file in that directory,
/// sync the temporary file, replace `path` with it, and sync the directory.
/// If the call made directories, it then syncs each directory above the
/// directory of `path` up to the parent of the highest new directory, deepest
/// first. On Unix, after the call returns, a power loss cannot lose the new
/// bytes of the file or the entry of a new directory. On Windows, a directory
/// has no sync in this module: the file sync occurs, but a power loss can
/// lose the rename and the entry of a new directory.
///
/// If a step fails, the steps after it do not occur. A failed sync gives
/// `sync <path>` and the I/O error. If a directory sync fails, `path` has the
/// new bytes, but a power loss can lose them.
pub fn write_synced_file(path: &Utf8Path, bytes: &[u8]) -> anyhow::Result<()> {
    let (temporary, parent, first_made) = write_temporary(path, bytes)?;
    drop(crate::test_probes::at("state_synced"));
    temporary.as_file().sync_all().map_err(sync_failure(path))?;
    replace_file(temporary, path)?;
    let last = first_made.and_then(Utf8Path::parent).unwrap_or(parent);
    for directory in parent.ancestors() {
        sync_parent(directory)?;
        if directory == last {
            break;
        }
    }
    Ok(())
}

/// Removes the file at `path` and then syncs its directory. On Unix, after
/// the call returns, a power loss cannot bring the file back. On Windows, a
/// power loss can bring the file back. A file that does not exist is a
/// success with no sync. A failed remove gives `remove <path>` and the I/O
/// error, and a failed sync gives `sync <path>` and the I/O error. If the
/// sync fails, the file is removed, but a power loss can bring it back.
pub fn remove_synced_file(path: &Utf8Path) -> anyhow::Result<()> {
    let parent = parent_of(path)?;
    match std::fs::remove_file(crate::long_path::extended(path)?) {
        Ok(()) => sync_parent(parent),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(crate::refusal::caused(error, format!("remove {path}"))),
    }
}

/// Writes `bytes` to the file at `path` in a staged tree, with a temporary
/// file and a replace, and does not sync. The caller syncs the tree with
/// [`sync_tree`] one time, after all of its writes and before it uses the
/// tree. That sync also covers each directory that this call makes.
pub(in crate::storage::local) fn write_staged_file(
    path: &Utf8Path,
    bytes: &[u8],
) -> anyhow::Result<()> {
    let (temporary, _, _) = write_temporary(path, bytes)?;
    replace_file(temporary, path)
}

/// A temporary file with `bytes` in the directory of `path`, that directory,
/// and the highest directory that this function made, if it made one. The
/// function makes the directory of `path` and each directory above it that
/// does not exist.
fn write_temporary<'a>(
    path: &'a Utf8Path,
    bytes: &[u8],
) -> anyhow::Result<(tempfile::NamedTempFile, &'a Utf8Path, Option<&'a Utf8Path>)> {
    let parent = parent_of(path)?;
    let mut first_made = None;
    for directory in parent.ancestors() {
        if directory.as_str().is_empty() || crate::long_path::extended(directory)?.try_exists()? {
            break;
        }
        first_made = Some(directory);
    }
    std::fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(crate::long_path::extended(parent)?)?;
    temporary.write_all(bytes)?;
    Ok((temporary, parent, first_made))
}

fn parent_of(path: &Utf8Path) -> anyhow::Result<&Utf8Path> {
    path.parent()
        .ok_or_else(|| crate::refusal::fault!("{path} has no parent directory"))
}

/// Syncs `directory`. A relative path of one name, such as the file that git
/// gives to a merge driver, has an empty parent: the current directory.
fn sync_parent(directory: &Utf8Path) -> anyhow::Result<()> {
    let directory = if directory.as_str().is_empty() {
        Utf8Path::new(".")
    } else {
        directory
    };
    drop(crate::test_probes::at("state_synced"));
    #[cfg(unix)]
    std::fs::File::open(directory)
        .and_then(|opened| opened.sync_all())
        .map_err(sync_failure(directory))?;
    #[cfg(not(unix))]
    let _ = directory;
    Ok(())
}

/// A failed sync of `path`, with the path and the I/O error.
fn sync_failure(path: &Utf8Path) -> impl FnOnce(std::io::Error) -> anyhow::Error + '_ {
    move |error| crate::refusal::caused(error, format!("sync {path}"))
}

/// Replaces the file at `target` with `temporary`, a file in the same
/// directory. The target has all of the new bytes or keeps its old bytes. A
/// handle that a reader has open on the target does not make the replace fail
/// when that handle lets other handles delete the file, as `std::fs` handles
/// do. The temporary file stays open until the rename returns. After a
/// failure, the temporary file is deleted.
///
/// This function does not sync. [`write_synced_file`] syncs the temporary
/// file before the call and the directories after it. [`write_staged_file`]
/// does not sync: its caller syncs the staged tree after all of its writes.
fn replace_file(temporary: tempfile::NamedTempFile, target: &Utf8Path) -> anyhow::Result<()> {
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
    drop(crate::test_probes::at("state_synced"));
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
            drop(crate::test_probes::at("state_synced"));
            file.sync_all().map_err(|error| {
                crate::refusal::fault!("sync publication file {child}: {error}")
            })?;
        }
    }
    sync_directory(path)
}
