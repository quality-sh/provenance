//! A synced write or removal of a file with a check of the file before the
//! one step that changes it.

use super::{
    keep, parent_of, sync_directories, sync_failure, sync_parent, write_temporary, ChangedUnsynced,
};
use std::path::Path;

#[cfg(test)]
mod tests;

/// The length of the random part of a temporary file name.
const RANDOM_LENGTH: usize = 6;
const SUFFIX: &str = ".tmp";

/// Replaces the file at `path` with `bytes` in one rename, after `check`.
///
/// The steps, in order: remove each regular file in the directory of `path`
/// whose name is `temporary_prefix`, six letters or digits, and `.tmp`, write
/// the temporary file in that directory with a name of that form, set its
/// permissions, sync it, call `check`, rename the temporary file onto `path`
/// in one step, and sync the directories as [`super::write_synced_file`]
/// does. The removal does not know which call made a file. The caller gives
/// a prefix that only its own temporary files have. A stop before the rename
/// leaves `path` with its old bytes and can leave the temporary file. A stop
/// after the rename leaves `path` with all of the new bytes.
///
/// `permissions` are the permissions of the new file. With `None`, the new
/// file gets the permissions that `std::fs::File::create` gives. On Windows,
/// the rename also replaces a read-only file.
///
/// An error tells that `path` keeps its old bytes. When `check` fails, the
/// temporary file is deleted and the call gives the error of `check`. After
/// `check` returns, the call does no step on `path` before the rename. A
/// failed rename gives the I/O error and deletes the temporary file. After
/// the rename, the call gives the permissions of the new file. If a
/// directory sync fails, the inner error gives them with the
/// [`ChangedUnsynced`]: `path` has the new bytes, but a power loss can lose
/// them.
pub fn write_synced_file_checked(
    path: &Path,
    bytes: &[u8],
    temporary_prefix: &str,
    permissions: Option<&std::fs::Permissions>,
    check: impl FnOnce() -> anyhow::Result<()>,
) -> anyhow::Result<Result<std::fs::Permissions, (std::fs::Permissions, ChangedUnsynced)>> {
    remove_stopped_temporaries(path, temporary_prefix)?;
    let mut builder = tempfile::Builder::new();
    builder
        .prefix(temporary_prefix)
        .suffix(SUFFIX)
        .rand_bytes(RANDOM_LENGTH);
    #[cfg(unix)]
    if permissions.is_none() {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o666));
    }
    let (temporary, parent, first_made) = write_temporary(path, bytes, &builder)?;
    let target = crate::long_path::extended(path)?;
    let (file, temporary) = keep(temporary)?;
    let prepared = (|| -> anyhow::Result<std::fs::Permissions> {
        if let Some(permissions) = permissions {
            file.set_permissions(permissions.clone()).map_err(|error| {
                let step = format!("set the permissions of {}", temporary.display());
                crate::refusal::caused(error, step)
            })?;
        }
        file.sync_all().map_err(sync_failure(path))?;
        let permissions = file
            .metadata()
            .map_err(|error| {
                crate::refusal::caused(error, format!("read {}", temporary.display()))
            })?
            .permissions();
        check()?;
        Ok(permissions)
    })();
    let permissions = match prepared {
        Ok(permissions) => permissions,
        Err(error) => {
            drop(file);
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
    };
    #[cfg(windows)]
    rename_kept_over_read_only(file, &temporary, &target)?;
    #[cfg(not(windows))]
    super::rename_kept(file, &temporary, &target)?;
    Ok(match unsynced(sync_directories(parent, first_made)) {
        Ok(()) => Ok(permissions),
        Err(unsynced) => Err((permissions, unsynced)),
    })
}

/// Removes the file at `path` in one remove, after `check`, and then syncs
/// its directory.
///
/// After `check` returns, the call does no step on `path` before the remove.
/// A file that does not exist is a success with no sync. An error tells
/// that `path` is as it was before the call: the error of `check`, or
/// `remove <path>` and the I/O error. After the remove, the call gives
/// `Ok`. If the directory sync fails, the inner error is the
/// [`ChangedUnsynced`]: the file is removed, but a power loss can bring it
/// back. On Windows, a power loss can bring the file back.
pub fn remove_synced_file_checked(
    path: &Path,
    check: impl FnOnce() -> anyhow::Result<()>,
) -> anyhow::Result<Result<(), ChangedUnsynced>> {
    let parent = parent_of(path)?;
    let target = crate::long_path::extended(path)?;
    check()?;
    match std::fs::remove_file(target) {
        Ok(()) => Ok(unsynced(sync_parent(parent))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Ok(())),
        Err(error) => Err(crate::refusal::caused(
            error,
            format!("remove {}", path.display()),
        )),
    }
}

/// The result of a directory sync after the file changed.
fn unsynced(synced: anyhow::Result<()>) -> Result<(), ChangedUnsynced> {
    synced.map_err(|error| {
        error
            .downcast::<ChangedUnsynced>()
            .unwrap_or_else(ChangedUnsynced)
    })
}

/// Removes each regular file in the directory of `path` whose name is
/// `prefix`, six letters or digits, and `.tmp`, whichever call or person
/// made it. A directory that does not exist has no such file.
fn remove_stopped_temporaries(path: &Path, prefix: &str) -> anyhow::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let directory = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    let entries = match crate::long_path::extended(directory).and_then(std::fs::read_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(list_failure(directory, error)),
    };
    for entry in entries {
        let entry = entry.map_err(|error| list_failure(directory, error))?;
        let is_temporary = entry
            .file_name()
            .to_str()
            .is_some_and(|name| is_temporary_name(name, prefix));
        if !is_temporary || !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        match std::fs::remove_file(entry.path()) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                let file = entry.path();
                return Err(crate::refusal::caused(
                    error,
                    format!("remove {}", file.display()),
                ));
            }
        }
    }
    Ok(())
}

/// A failed list of `directory`, with the path and the I/O error.
fn list_failure(directory: &Path, error: std::io::Error) -> anyhow::Error {
    crate::refusal::caused(error, format!("list directory {}", directory.display()))
}

/// Whether `name` is `prefix`, six letters or digits, and `.tmp`.
fn is_temporary_name(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .and_then(|rest| rest.strip_suffix(SUFFIX))
        .is_some_and(|random| {
            random.len() == RANDOM_LENGTH && random.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

/// The flags of a Windows rename that replaces a read-only target.
#[cfg(windows)]
const RENAME_FLAGS: u32 = REPLACE_IF_EXISTS | POSIX_SEMANTICS | IGNORE_READONLY_ATTRIBUTE;
#[cfg(windows)]
const REPLACE_IF_EXISTS: u32 = 0x1;
#[cfg(windows)]
const POSIX_SEMANTICS: u32 = 0x2;
#[cfg(windows)]
const IGNORE_READONLY_ATTRIBUTE: u32 = 0x40;

/// Renames the kept temporary file `path` onto `target` as
/// [`super::rename_kept`] does, and also replaces a read-only `target`. A
/// file system that does not know this rename gets the rename of `std::fs`,
/// which does not replace a read-only `target`. After a failure, the
/// temporary file is deleted.
#[cfg(windows)]
fn rename_kept_over_read_only(
    file: std::fs::File,
    path: &std::path::Path,
    target: &std::path::Path,
) -> Result<(), super::ReplaceFailed> {
    let renamed = match rename_ignoring_read_only(path, target) {
        Err(error) if unknown_to_the_file_system(&error) => std::fs::rename(path, target),
        renamed => renamed,
    };
    drop(file);
    renamed.map_err(|error| {
        let _ = std::fs::remove_file(path);
        super::ReplaceFailed(error)
    })
}

/// Whether the file system refused the rename class, and not the rename.
#[cfg(windows)]
fn unknown_to_the_file_system(error: &std::io::Error) -> bool {
    use windows_sys::Win32::Foundation::{
        ERROR_INVALID_FUNCTION, ERROR_INVALID_PARAMETER, ERROR_NOT_SUPPORTED,
    };
    [
        ERROR_INVALID_PARAMETER,
        ERROR_NOT_SUPPORTED,
        ERROR_INVALID_FUNCTION,
    ]
    .into_iter()
    .any(|code| i32::try_from(code).ok() == error.raw_os_error())
}

/// Renames `path` onto `target` with `FileRenameInfoEx`, which replaces
/// `target` in one step also when it is read-only.
#[cfg(windows)]
fn rename_ignoring_read_only(
    path: &std::path::Path,
    target: &std::path::Path,
) -> std::io::Result<()> {
    use crate::operations::files::rename_info::RenameInfo;
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        FileRenameInfoEx, SetFileInformationByHandle, DELETE, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };
    let source = std::fs::OpenOptions::new()
        .access_mode(DELETE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    let info = RenameInfo::new(target.as_os_str(), RENAME_FLAGS, None)?;
    // SAFETY: `info` holds a whole `FILE_RENAME_INFO` of `info.size()` bytes,
    // and `source` is an open handle, until the call returns.
    let done = unsafe {
        SetFileInformationByHandle(
            source.as_raw_handle() as HANDLE,
            FileRenameInfoEx,
            info.as_ptr(),
            info.size(),
        )
    };
    if done == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
