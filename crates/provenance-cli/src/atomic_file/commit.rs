use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub(super) fn displace_to_backup(path: &Path) -> std::io::Result<PathBuf> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    for attempt in 0..100_u8 {
        let backup = parent.join(format!(
            ".{name}.provenance-{}-{attempt}.backup",
            std::process::id(),
        ));
        match rename_no_replace(path, &backup) {
            Ok(()) => return Ok(backup),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        ErrorKind::AlreadyExists,
        "could not allocate backup file",
    ))
}

pub(super) fn remove_preserved(path: &Path) -> std::io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        return std::fs::remove_dir_all(path);
    }
    #[cfg(windows)]
    if metadata.file_type().is_symlink() && std::fs::metadata(path).is_ok_and(|item| item.is_dir())
    {
        return std::fs::remove_dir(path);
    }
    #[cfg(windows)]
    if metadata.permissions().readonly() && metadata.is_file() {
        let mut permissions = metadata.permissions();
        permissions.set_readonly(false);
        std::fs::set_permissions(path, permissions)?;
    }
    std::fs::remove_file(path)
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
pub(super) fn rename_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        from,
        rustix::fs::CWD,
        to,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)
}

#[cfg(target_os = "macos")]
pub(super) fn rename_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int, c_uint};
    use std::os::unix::ffi::OsStrExt;

    extern "C" {
        fn renameatx_np(
            from_dir: c_int,
            from: *const c_char,
            to_dir: c_int,
            to: *const c_char,
            flags: c_uint,
        ) -> c_int;
    }

    let from = CString::new(from.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(ErrorKind::InvalidInput))?;
    let to = CString::new(to.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::from(ErrorKind::InvalidInput))?;
    let result = unsafe { renameatx_np(-2, from.as_ptr(), -2, to.as_ptr(), 0x0000_0004) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(windows)]
pub(super) fn rename_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::rename(from, to)
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "macos",
    windows
)))]
pub(super) fn rename_no_replace(_from: &Path, _to: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "atomic no-replace rename is unavailable on this platform",
    ))
}
