//! A Windows rename that also replaces a read-only target.

use crate::storage::local::durability::ReplaceFailed;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use windows_sys::Win32::Foundation::{
    ERROR_INVALID_FUNCTION, ERROR_INVALID_PARAMETER, ERROR_NOT_SUPPORTED, HANDLE,
};
use windows_sys::Win32::Storage::FileSystem::{
    FileRenameInfoEx, SetFileInformationByHandle, DELETE, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_RENAME_INFO, FILE_RENAME_INFO_0, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE,
};

const REPLACE_IF_EXISTS: u32 = 0x1;
const POSIX_SEMANTICS: u32 = 0x2;
const IGNORE_READONLY_ATTRIBUTE: u32 = 0x40;

/// Renames the kept temporary file `path` onto `target` as
/// [`super::super::rename_kept`] does, and also replaces a read-only
/// `target`. A file system that does not know this rename gets the rename of
/// `std::fs`, which does not replace a read-only `target`. After a failure,
/// the temporary file is deleted.
pub(super) fn rename_kept(
    file: std::fs::File,
    path: &Path,
    target: &Path,
) -> Result<(), ReplaceFailed> {
    let renamed = match rename_ignoring_read_only(path, target) {
        Err(error) if unknown_to_the_file_system(&error) => std::fs::rename(path, target),
        renamed => renamed,
    };
    drop(file);
    renamed.map_err(|error| {
        super::delete_temporary(path);
        ReplaceFailed(error)
    })
}

/// Whether the file system refused the rename class, and not the rename.
fn unknown_to_the_file_system(error: &std::io::Error) -> bool {
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
fn rename_ignoring_read_only(path: &Path, target: &Path) -> std::io::Result<()> {
    use std::mem::{offset_of, size_of};
    let source = std::fs::OpenOptions::new()
        .access_mode(DELETE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)?;
    let name: Vec<u16> = target.as_os_str().encode_wide().collect();
    let name_bytes = name
        .len()
        .checked_mul(size_of::<u16>())
        .ok_or(std::io::ErrorKind::InvalidInput)?;
    let buffer_size = offset_of!(FILE_RENAME_INFO, FileName)
        .checked_add(name_bytes)
        .and_then(|size| size.checked_add(size_of::<u16>()))
        .ok_or(std::io::ErrorKind::InvalidInput)?;
    let mut storage = vec![0_usize; buffer_size.div_ceil(size_of::<usize>())];
    let info = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    let name_length = u32::try_from(name_bytes).map_err(|_| std::io::ErrorKind::InvalidInput)?;
    let size = u32::try_from(buffer_size).map_err(|_| std::io::ErrorKind::InvalidInput)?;
    // SAFETY: `storage` has room for the structure and the name with its
    // terminating zero, and it lives until the call returns.
    let done = unsafe {
        info.write(FILE_RENAME_INFO {
            Anonymous: FILE_RENAME_INFO_0 {
                Flags: REPLACE_IF_EXISTS | POSIX_SEMANTICS | IGNORE_READONLY_ATTRIBUTE,
            },
            RootDirectory: 0,
            FileNameLength: name_length,
            FileName: [0],
        });
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            std::ptr::addr_of_mut!((*info).FileName).cast::<u16>(),
            name.len(),
        );
        SetFileInformationByHandle(
            source.as_raw_handle() as HANDLE,
            FileRenameInfoEx,
            info.cast_const().cast(),
            size,
        )
    };
    if done == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
