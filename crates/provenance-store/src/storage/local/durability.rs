use crate::layout::ProvenanceLayout;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;
use std::io::Write;
use std::path::{Path, PathBuf};

mod checked;
pub(in crate::storage::local) mod temporary_name;
pub use checked::{remove_synced_file_checked, write_synced_file_checked};

/// The prefix of the name of the temporary file of a write of one state key,
/// in the cache. The section entry removes each file of this name form.
pub(in crate::storage::local) const STATE_TEMPORARY_PREFIX: &str = ".state-key.provenance-";

/// A failure of [`replace_file`]. Its text is the start of the text of a
/// `tempfile` persist failure. Its source is the I/O error, so that the
/// chain of the error gives the I/O error one time.
#[derive(Debug, thiserror::Error)]
#[error("failed to persist temporary file")]
pub struct ReplaceFailed(#[source] std::io::Error);

/// A failure after the file at the path changed: a sync of its directory.
///
/// [`write_synced_file`] and [`remove_synced_file`] give it in the chain of
/// their error. [`write_synced_file_checked`] and
/// [`remove_synced_file_checked`] give it in their inner result.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct ChangedUnsynced(pub anyhow::Error);

/// Writes `bytes` to the file at `path` in this order: make the directory of
/// `path` if it does not exist, write a temporary file in that directory,
/// sync the temporary file, replace `path` with it, and sync the directory.
/// If the call made directories, it then syncs each directory above the
/// directory of `path` up to the parent of the highest new directory, deepest
/// first. It does not sync a directory that was there before the call, above
/// the directory of `path`. On Unix, after the call returns, a power loss
/// cannot lose the new bytes of the file or the entry of a directory that the
/// call made, if each directory that was there before the call has its entry
/// on the disk. On Windows, a directory has no sync in this module: the file
/// sync occurs, but a power loss can lose the rename and the entry of a new
/// directory.
///
/// If a step fails, the steps after it do not occur, no temporary file stays,
/// and the error names the step and its path, except a failed rename. A failed
/// sync gives `sync <path>` and the I/O error. If a directory sync fails,
/// `path` has the new bytes, but a power loss can lose them: the error is a
/// [`ChangedUnsynced`].
pub fn write_synced_file(path: &Utf8Path, bytes: &[u8]) -> anyhow::Result<()> {
    write_synced_file_named(path, bytes, &tempfile::Builder::new())
}

/// Writes `bytes` to the file at `path` as [`write_synced_file`] does, with a
/// temporary file whose name `builder` gives.
pub(in crate::storage::local) fn write_synced_file_named(
    path: &Utf8Path,
    bytes: &[u8],
    builder: &tempfile::Builder<'_, '_>,
) -> anyhow::Result<()> {
    let (temporary, parent, first_made) = write_temporary(path.as_std_path(), bytes, builder)?;
    install_synced(temporary, path, parent, first_made)
}

/// Writes `bytes` to the state file at `path` as [`write_synced_file`] does,
/// with a temporary file in the cache of `layout` and not in the state
/// directory. The state directory is in git, so a stopped write must leave
/// no file there. The name of the temporary file starts with
/// [`STATE_TEMPORARY_PREFIX`], and the rename moves it from the cache onto
/// `path`. A whole-set write also renames from the cache into the state
/// directory, so both are on one file system.
///
/// The caller holds the section lock of `layout`. The section entry removes
/// each temporary file of this name form, so with no lock it can remove the
/// file of this write before the rename.
///
/// After the rename, the call syncs the directory of `path` and each
/// directory that it made, as [`write_synced_file`] does. It does not sync
/// the cache. A stop or a power loss before the rename leaves `path` with
/// its old bytes and can leave the temporary file in the cache. A power loss
/// after the rename can also leave the temporary file in the cache, because
/// the removal of its name from the cache is not synced. The next section
/// entry removes such a file.
pub(crate) fn write_synced_state_file(
    layout: &ProvenanceLayout,
    path: &Utf8Path,
    bytes: &[u8],
) -> anyhow::Result<()> {
    let (parent, first_made) = make_parent(path.as_std_path())?;
    let temporary = write_state_temporary(layout, path, bytes)?;
    install_synced(temporary, path, parent, first_made)
}

/// A temporary file with `bytes` for the state file at `path`, in the cache
/// of `layout`, so that a write of a state key adds no file to the state
/// directory.
#[rule("rule_one_key_write_adds_no_other_state_file")]
fn write_state_temporary(
    layout: &ProvenanceLayout,
    path: &Utf8Path,
    bytes: &[u8],
) -> anyhow::Result<tempfile::NamedTempFile> {
    let cache = layout.cache_dir();
    std::fs::create_dir_all(&cache).map_err(step_failure("make directory", cache.as_std_path()))?;
    let builder = temporary_name::builder(STATE_TEMPORARY_PREFIX);
    write_temporary_in(cache.as_std_path(), path.as_std_path(), bytes, &builder)
}

/// Syncs `temporary`, replaces `path` with it, and syncs `parent`, the
/// directory of `path`, and each directory up to the parent of `first_made`.
fn install_synced(
    temporary: tempfile::NamedTempFile,
    path: &Utf8Path,
    parent: &Path,
    first_made: Option<&Path>,
) -> anyhow::Result<()> {
    temporary
        .as_file()
        .sync_all()
        .map_err(sync_failure(path.as_std_path()))?;
    crate::test_probes::at("synced_file_before_rename")?;
    replace_file(temporary, path)?;
    sync_directories(parent, first_made)
}

/// Syncs the file at `path`, which already has the bytes of a write, and then
/// syncs the directory of `path`.
///
/// It does not sync a directory above the directory of `path`. The call that
/// makes a directory syncs its entry, so the entry of a directory above is on
/// the disk only if the call that made it synced it.
pub fn sync_written_file(path: &Utf8Path) -> anyhow::Result<()> {
    open_to_sync(path)
        .and_then(|file| file.sync_all())
        .map_err(sync_failure(path.as_std_path()))?;
    sync_directories(parent_of(path.as_std_path())?, None)
}

/// Syncs `parent`, and then each directory above it up to the parent of
/// `first_made`, the highest directory that a write made, deepest first.
fn sync_directories(parent: &Path, first_made: Option<&Path>) -> anyhow::Result<()> {
    let last = first_made.and_then(Path::parent).unwrap_or(parent);
    for directory in parent.ancestors() {
        sync_parent(directory).map_err(ChangedUnsynced)?;
        if directory == last {
            break;
        }
    }
    Ok(())
}

/// Removes the file at `path` and then syncs its directory, as
/// [`remove_synced_file_checked`] does with no check.
///
/// On Unix, after the call returns, a power loss cannot bring the file back.
/// On Windows, a power loss can bring the file back. A file that does not
/// exist is a success with no sync. A failed remove gives `remove <path>` and
/// the I/O error, and a failed sync gives `sync <path>` and the I/O error. If
/// the sync fails, the file is removed, but a power loss can bring it back:
/// the error is a [`ChangedUnsynced`].
///
/// True when it removed a file.
pub fn remove_synced_file(path: &Utf8Path) -> anyhow::Result<bool> {
    remove_synced_file_checked(path.as_std_path(), || Ok(()))?.map_err(anyhow::Error::from)
}

/// Writes `bytes` to the file at `path` in a staged tree, with a temporary
/// file and a replace, and does not sync. The caller syncs the tree with
/// [`sync_tree`] one time, after all of its writes and before it uses the
/// tree. That sync also covers each directory that this call makes.
pub(in crate::storage::local) fn write_staged_file(
    path: &Utf8Path,
    bytes: &[u8],
) -> anyhow::Result<()> {
    let (temporary, _, _) = write_temporary(path.as_std_path(), bytes, &tempfile::Builder::new())?;
    replace_file(temporary, path)
}

/// A temporary file with `bytes` in the directory of `path`, with the name
/// that `builder` gives, that directory, and the highest directory that
/// this function made, if it made one. The function makes the directory of
/// `path` and each directory above it that does not exist.
fn write_temporary<'a>(
    path: &'a Path,
    bytes: &[u8],
    builder: &tempfile::Builder<'_, '_>,
) -> anyhow::Result<(tempfile::NamedTempFile, &'a Path, Option<&'a Path>)> {
    let (parent, first_made) = make_parent(path)?;
    let temporary = write_temporary_in(parent, path, bytes, builder)?;
    Ok((temporary, parent, first_made))
}

/// Makes the directory of `path` and each directory above it that does not
/// exist. Gives that directory and the highest directory that the call made,
/// if it made one.
fn make_parent(path: &Path) -> anyhow::Result<(&Path, Option<&Path>)> {
    let parent = parent_of(path)?;
    let mut first_made = None;
    for directory in parent.ancestors() {
        if directory.as_os_str().is_empty() || directory_exists(directory)? {
            break;
        }
        first_made = Some(directory);
    }
    std::fs::create_dir_all(parent).map_err(step_failure("make directory", parent))?;
    Ok((parent, first_made))
}

/// A temporary file with `bytes` in `directory`, with the name that
/// `builder` gives. An error names `path`, the file that the temporary file
/// is for.
fn write_temporary_in(
    directory: &Path,
    path: &Path,
    bytes: &[u8],
    builder: &tempfile::Builder<'_, '_>,
) -> anyhow::Result<tempfile::NamedTempFile> {
    let mut temporary = crate::long_path::extended(directory)
        .and_then(|directory| builder.tempfile_in(directory))
        .map_err(step_failure("make a temporary file for", path))?;
    temporary
        .write_all(bytes)
        .map_err(step_failure("write a temporary file for", path))?;
    Ok(temporary)
}

/// Tells if `directory` exists. A failure gives `find directory <path>` and
/// the I/O error.
fn directory_exists(directory: &Path) -> anyhow::Result<bool> {
    crate::long_path::extended(directory)
        .and_then(|extended| extended.try_exists())
        .map_err(step_failure("find directory", directory))
}

fn parent_of(path: &Path) -> anyhow::Result<&Path> {
    path.parent()
        .ok_or_else(|| crate::refusal::fault!("{} has no parent directory", path.display()))
}

/// Syncs `directory`. A relative path of one name, such as the file that git
/// gives to a merge driver, has an empty parent: the current directory.
fn sync_parent(directory: &Path) -> anyhow::Result<()> {
    let directory = if directory.as_os_str().is_empty() {
        Path::new(".")
    } else {
        directory
    };
    #[cfg(unix)]
    std::fs::File::open(directory)
        .and_then(|opened| opened.sync_all())
        .map_err(sync_failure(directory))?;
    #[cfg(not(unix))]
    let _ = directory;
    Ok(())
}

/// Opens the file at `path` so that its sync can run. A Windows sync of a
/// file needs write access, and a handle with read access only gets
/// `ERROR_ACCESS_DENIED`. Unix syncs a handle with read access only.
fn open_to_sync(path: &Utf8Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .read(true)
        .write(cfg!(windows))
        .open(path)
}

/// A failed sync of `path`, with the path and the I/O error.
fn sync_failure(path: &Path) -> impl FnOnce(std::io::Error) -> anyhow::Error + '_ {
    step_failure("sync", path)
}

/// A failed `step` on `path`, with the step, the path and the I/O error.
fn step_failure<'a>(
    step: &'a str,
    path: &'a Path,
) -> impl FnOnce(std::io::Error) -> anyhow::Error + 'a {
    move |error| crate::refusal::caused(error, format!("{step} {}", path.display()))
}

/// Replaces the file at `target` with `temporary`, a file on the same file
/// system. The target has all of the new bytes or keeps its old bytes. A
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
    let (file, path) = keep(temporary)?;
    rename_kept(file, &path, &target)?;
    Ok(())
}

/// Keeps `temporary` after its handle drops, and gives the open file and
/// its path. On Windows, `keep` clears the temporary attribute that
/// `tempfile` sets. A rename keeps the attributes of the file, and with that
/// attribute the system can keep the bytes in memory and not write them to
/// the disk. After a failure, the temporary file is deleted.
fn keep(temporary: tempfile::NamedTempFile) -> Result<(std::fs::File, PathBuf), ReplaceFailed> {
    temporary
        .keep()
        .map_err(|failure| ReplaceFailed(failure.error))
}

/// Renames the kept temporary file `path` onto `target`, as
/// [`replace_file`] does, and closes `file`, the open temporary file, after
/// the rename returns.
fn rename_kept(file: std::fs::File, path: &Path, target: &Path) -> Result<(), ReplaceFailed> {
    let renamed = std::fs::rename(path, target);
    drop(file);
    renamed.map_err(|error| {
        let _ = std::fs::remove_file(path);
        ReplaceFailed(error)
    })
}

/// Flushes directory metadata where the platform supports it.
///
/// Windows has no equivalent call in this module. On Windows, file contents are flushed, but a
/// sudden power loss can lose a recent directory entry or rename. Process-crash recovery remains
/// available because the operating system keeps the directory changes.
pub fn sync_directory(path: &Utf8Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    sync_opened_directory(&std::fs::File::open(path)?)?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Flushes the metadata of the open directory `directory`, as
/// [`sync_directory`] does: on Windows, the call does nothing.
pub fn sync_opened_directory(directory: &std::fs::File) -> std::io::Result<()> {
    #[cfg(unix)]
    directory.sync_all()?;
    #[cfg(not(unix))]
    let _ = directory;
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
            let file = open_to_sync(&child).map_err(|error| {
                crate::refusal::fault!("sync publication file {child}: {error}")
            })?;
            file.sync_all().map_err(|error| {
                crate::refusal::fault!("sync publication file {child}: {error}")
            })?;
        }
    }
    sync_directory(path)
}
