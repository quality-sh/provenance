//! State-tree file helpers split out of the publication module.
//!
//! These are the access and durability helpers that sit beside the
//! publication lock: a consistent read-only snapshot of the state tree,
//! the copy and sync primitives the snapshot and import staging use, and
//! the accessor that puts an arbitrary state path under its repository's
//! publication lock. The lock protocol itself stays in the parent module.

use crate::layout::ProvenanceLayout;
use anyhow::anyhow;
use camino::{Utf8Path, Utf8PathBuf};

pub struct StateSnapshot {
    _directory: tempfile::TempDir,
    layout: ProvenanceLayout,
}

impl StateSnapshot {
    pub const fn layout(&self) -> &ProvenanceLayout {
        &self.layout
    }
}

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
            .map_err(|path| anyhow!("publication path is not UTF-8: {}", path.display()))?;
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
                .map_err(|error| anyhow!("sync publication file {child}: {error}"))?;
            file.sync_all()
                .map_err(|error| anyhow!("sync publication file {child}: {error}"))?;
        }
    }
    sync_directory(path)
}

pub fn snapshot_state(layout: &ProvenanceLayout) -> anyhow::Result<StateSnapshot> {
    super::with_repository_publication(layout, || {
        let directory = tempfile::tempdir()?;
        let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf())
            .map_err(|path| anyhow!("snapshot path is not UTF-8: {}", path.display()))?;
        let snapshot_layout = ProvenanceLayout::new(root);
        copy_tree(&layout.state_dir(), &snapshot_layout.state_dir())?;
        Ok(StateSnapshot {
            _directory: directory,
            layout: snapshot_layout,
        })
    })
}

pub fn copy_tree(source: &Utf8Path, destination: &Utf8Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_child = Utf8PathBuf::from_path_buf(entry.path())
            .map_err(|path| anyhow!("state path is not UTF-8: {}", path.display()))?;
        let destination_child = destination.join(entry.file_name().to_string_lossy().as_ref());
        let file_type = std::fs::symlink_metadata(&source_child)?.file_type();
        if file_type.is_dir() {
            copy_tree(&source_child, &destination_child)?;
        } else if file_type.is_file() {
            std::fs::copy(source_child, destination_child)?;
        } else {
            anyhow::bail!("unsupported state entry: {source_child}");
        }
    }
    Ok(())
}

#[allow(clippy::redundant_pub_crate)]
pub(crate) fn with_state_path_access<R>(
    path: &Utf8Path,
    operation: impl FnOnce() -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    let Some(state_dir) = path.ancestors().find(|ancestor| {
        ancestor.file_name() == Some("state")
            && ancestor.parent().and_then(Utf8Path::file_name) == Some(".provenance")
    }) else {
        return operation();
    };
    let root = state_dir
        .parent()
        .and_then(Utf8Path::parent)
        .ok_or_else(|| anyhow!("state path has no repository root"))?;
    super::with_repository_publication(&ProvenanceLayout::new(root), operation)
}
