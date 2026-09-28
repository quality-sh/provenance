use camino::{Utf8Path, Utf8PathBuf};

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
        .map_err(|error| anyhow::anyhow!("list publication tree {path}: {error}"))?
    {
        let entry = entry?;
        let child = Utf8PathBuf::from_path_buf(entry.path())
            .map_err(|path| anyhow::anyhow!("publication path is not UTF-8: {}", path.display()))?;
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
                .map_err(|error| anyhow::anyhow!("sync publication file {child}: {error}"))?;
            file.sync_all()
                .map_err(|error| anyhow::anyhow!("sync publication file {child}: {error}"))?;
        }
    }
    sync_directory(path)
}
