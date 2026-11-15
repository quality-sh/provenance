//! A synced write of a file with a check of the target before the rename.

use super::{keep, rename_kept, sync_directories, sync_failure, write_temporary};
use camino::Utf8Path;

/// The length of the random part of a temporary file name.
const RANDOM_LENGTH: usize = 6;
const SUFFIX: &str = ".tmp";

/// Replaces the file at `path` with `bytes` in one rename, after `check`.
///
/// The steps, in order: remove each temporary file of `temporary_prefix`
/// that a stopped call left in the directory of `path`, write the temporary
/// file in that directory, set its permissions, sync it, call `check`, rename
/// the temporary file onto `path` in one step, and sync the directories as
/// [`super::write_synced_file`] does. The function gives the permissions of
/// the new file.
///
/// The name of the temporary file is `temporary_prefix`, six letters or
/// digits, and `.tmp`. Only a regular file with a name of that form is a
/// temporary file of a stopped call. A stop before the rename leaves `path`
/// with its old bytes and can leave the temporary file. A stop after the
/// rename leaves `path` with all of the new bytes.
///
/// `permissions` are the permissions of the new file. With `None`, the new
/// file gets the permissions that `std::fs::File::create` gives.
///
/// When `check` fails, the temporary file is deleted, `path` keeps its old
/// bytes, and the call gives the error of `check`. After `check` returns,
/// the call does no step on `path` before the rename. A failed rename gives
/// the I/O error, deletes the temporary file, and leaves `path` with its old
/// bytes.
pub fn write_synced_file_checked(
    path: &Utf8Path,
    bytes: &[u8],
    temporary_prefix: &str,
    permissions: Option<&std::fs::Permissions>,
    check: impl FnOnce() -> anyhow::Result<()>,
) -> anyhow::Result<std::fs::Permissions> {
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
    rename_kept(file, &temporary, &target)?;
    sync_directories(parent, first_made)?;
    Ok(permissions)
}

/// Removes each regular file in the directory of `path` whose name is
/// `prefix`, six letters or digits, and `.tmp`. A directory that does not
/// exist has no such file.
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
                return Err(crate::refusal::caused(error, format!("remove {}", file.display())));
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
