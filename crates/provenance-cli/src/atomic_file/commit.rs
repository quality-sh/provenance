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
        match crate::safe_fs::rename_no_replace(path, &backup) {
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
