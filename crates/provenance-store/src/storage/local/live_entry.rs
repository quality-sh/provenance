//! The live side of an install in place: the path of a key in the state
//! directory, the file system of the transaction, and the cause of a failed
//! replace on Windows.

use camino::{Utf8Path, Utf8PathBuf};

/// The path of `key` below `state`. The state directory and each part of
/// the key above the file that exists must be a real directory, and the
/// file, when it exists, a regular file. Thus a rename or a removal at the
/// path cannot follow a link out of the state directory. A part that does
/// not exist is a directory that the install makes.
pub fn live_file(state: &Utf8Path, key: &str) -> anyhow::Result<Utf8PathBuf> {
    let parts: Vec<&str> = key.split('/').collect();
    let mut path = state.to_owned();
    for (index, part) in std::iter::once("").chain(parts.iter().copied()).enumerate() {
        if !part.is_empty() {
            path.push(part);
        }
        let metadata = match std::fs::symlink_metadata(crate::long_path::extended(&path)?) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(key_path(state, key));
            }
            Err(error) => return Err(crate::refusal::caused(error, format!("read {path}"))),
            Ok(metadata) => metadata,
        };
        let expected = if index == parts.len() {
            metadata.is_file()
        } else {
            metadata.is_dir()
        };
        crate::write_error::ensure!(
            Fault,
            expected && !metadata.file_type().is_symlink(),
            "unsupported state entry: {path}"
        );
    }
    Ok(path)
}

/// Refuses a state directory that holds an entry that is not a regular file
/// or a real directory, such as a link, or whose name is not UTF-8, with the
/// path of the entry. Thus a whole set reads no link and leaves none.
pub fn ensure_plain_state_tree(directory: &Utf8Path) -> anyhow::Result<()> {
    let entries = match std::fs::read_dir(directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        entries => {
            entries.map_err(|error| crate::refusal::caused(error, format!("read {directory}")))?
        }
    };
    for entry in entries {
        let entry =
            entry.map_err(|error| crate::refusal::caused(error, format!("read {directory}")))?;
        let path = Utf8PathBuf::from_path_buf(entry.path()).map_err(|path| {
            crate::refusal::fault!("state path is not UTF-8: {}", path.display())
        })?;
        let kind = std::fs::symlink_metadata(&path)
            .map_err(|error| crate::refusal::caused(error, format!("read {path}")))?
            .file_type();
        if kind.is_dir() && !kind.is_symlink() {
            ensure_plain_state_tree(&path)?;
        } else {
            crate::write_error::ensure!(
                Fault,
                kind.is_file() && !kind.is_symlink(),
                "unsupported state entry: {path}"
            );
        }
    }
    Ok(())
}

/// The path of `key` below `directory`, with the separator of the platform
/// between its parts.
pub(super) fn key_path(directory: &Utf8Path, key: &str) -> Utf8PathBuf {
    key.split('/')
        .fold(directory.to_owned(), |path, part| path.join(part))
}

/// Refuses an install from `transaction` into `state` when the two
/// directories are on two file systems: a rename between them fails, and
/// the write must find that before its marker. When the state directory
/// does not exist, its nearest directory that exists is the one that the
/// install makes it in.
pub fn ensure_one_file_system(transaction: &Utf8Path, state: &Utf8Path) -> anyhow::Result<()> {
    let device = |path: &Utf8Path| {
        let existing = path
            .ancestors()
            .find(|ancestor| ancestor.is_dir())
            .unwrap_or(path);
        file_system(existing)
            .map_err(|error| crate::refusal::caused(error, format!("read {existing}")))
    };
    crate::write_error::ensure!(
        Fault,
        device(transaction)? == device(state)?,
        "install the state from the cache {transaction} in {state}: the cache and the state \
         directory are on two file systems, and the store writes the state only when they \
         are on one file system. Put .provenance/cache on the file system of \
         .provenance/state, and then send the call again"
    );
    Ok(())
}

#[cfg(unix)]
fn file_system(path: &Utf8Path) -> std::io::Result<u64> {
    use std::os::unix::fs::MetadataExt as _;
    std::fs::metadata(path).map(|metadata| metadata.dev())
}

/// The serial number of the volume that holds the directory at `path`.
#[cfg(windows)]
fn file_system(path: &Utf8Path) -> std::io::Result<u64> {
    use std::os::windows::fs::OpenOptionsExt as _;
    use std::os::windows::io::AsRawHandle as _;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };
    let directory = std::fs::OpenOptions::new()
        .access_mode(0)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(crate::long_path::extended(path)?)?;
    // SAFETY: the structure is plain data that the call fills, and
    // `directory` is an open handle until the call returns.
    let mut information: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    let done = unsafe {
        GetFileInformationByHandle(directory.as_raw_handle() as HANDLE, &mut information)
    };
    if done == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(u64::from(information.dwVolumeSerialNumber))
}

/// Whether the replace or the removal of the live file at `path` failed with
/// `code` because a different process has the file open with no delete
/// sharing. A sharing violation (32) always means that. An access denial
/// (5) means that only when an open of the file for deletion with full
/// sharing gives a sharing violation: a read-only attribute or an access
/// list that refuses the change lets that open pass or gives a denial.
#[cfg(windows)]
pub fn held_by_another_process(path: &Utf8Path, code: i32) -> bool {
    use std::os::windows::fs::OpenOptionsExt as _;
    use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION};
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };
    let sharing = i32::try_from(ERROR_SHARING_VIOLATION).ok();
    if Some(code) == sharing {
        return true;
    }
    if Some(code) != i32::try_from(ERROR_ACCESS_DENIED).ok() {
        return false;
    }
    let Ok(extended) = crate::long_path::extended(path) else {
        return false;
    };
    std::fs::OpenOptions::new()
        .access_mode(DELETE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .open(extended)
        .is_err_and(|error| error.raw_os_error() == sharing)
}

/// On Linux and macOS, a rename replaces a file that a different process
/// has open, so no failure is a held file.
#[cfg(not(windows))]
pub const fn held_by_another_process(_path: &Utf8Path, _code: i32) -> bool {
    false
}
