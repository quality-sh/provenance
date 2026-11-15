//! A synced write of a file with a check of the target before the rename.

use super::{keep, sync_directories, sync_failure, write_temporary, ChangedUnsynced};
use camino::Utf8Path;

#[cfg(test)]
mod tests;
#[cfg(windows)]
mod windows_rename;

/// The length of the random part of a temporary file name.
const RANDOM_LENGTH: usize = 6;
const SUFFIX: &str = ".tmp";

/// The file that [`write_synced_file_checked`] put at its path with the rename.
#[derive(Debug)]
#[must_use]
pub struct ReplacedFile {
    /// The permissions of the new file.
    pub permissions: std::fs::Permissions,
    /// The sync of the directories after the rename. An error tells that the
    /// path has the new bytes, but a power loss can lose them.
    pub synced: Result<(), ChangedUnsynced>,
}

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
/// the rename, the call gives a [`ReplacedFile`], also when a directory sync
/// fails.
pub fn write_synced_file_checked(
    path: &Utf8Path,
    bytes: &[u8],
    temporary_prefix: &str,
    permissions: Option<&std::fs::Permissions>,
    check: impl FnOnce() -> anyhow::Result<()>,
) -> anyhow::Result<ReplacedFile> {
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
            delete_temporary(&temporary);
            return Err(error);
        }
    };
    #[cfg(windows)]
    windows_rename::rename_kept(file, &temporary, &target)?;
    #[cfg(not(windows))]
    super::rename_kept(file, &temporary, &target)?;
    let synced = sync_directories(parent, first_made).map_err(|error| {
        error
            .downcast::<ChangedUnsynced>()
            .unwrap_or_else(ChangedUnsynced)
    });
    Ok(ReplacedFile {
        permissions,
        synced,
    })
}

/// Deletes the temporary file at `path`, also when it is read-only: Windows
/// does not delete a read-only file.
fn delete_temporary(path: &std::path::Path) {
    #[cfg(windows)]
    if let Ok(metadata) = std::fs::metadata(path) {
        let mut permissions = metadata.permissions();
        if permissions.readonly() {
            permissions.set_readonly(false);
            let _ = std::fs::set_permissions(path, permissions);
        }
    }
    let _ = std::fs::remove_file(path);
}

/// Removes each regular file in the directory of `path` whose name is
/// `prefix`, six letters or digits, and `.tmp`, whichever call or person
/// made it. A directory that does not exist has no such file.
fn remove_stopped_temporaries(path: &Utf8Path, prefix: &str) -> anyhow::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let directory = if parent.as_str().is_empty() {
        Utf8Path::new(".")
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
fn list_failure(directory: &Utf8Path, error: std::io::Error) -> anyhow::Error {
    crate::refusal::caused(error, format!("list directory {directory}"))
}

/// Whether `name` is `prefix`, six letters or digits, and `.tmp`.
fn is_temporary_name(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .and_then(|rest| rest.strip_suffix(SUFFIX))
        .is_some_and(|random| {
            random.len() == RANDOM_LENGTH && random.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}
