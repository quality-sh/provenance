//! Copies one installed skill directory into `.claude/skills`.
//!
//! A skill is a directory, not one file. The agent skills specification lets
//! a skill carry `scripts/`, `references/` and `assets/` beside its
//! `SKILL.md`, and an agent that reads the copy needs those files too. This
//! module thus copies each file below the canonical skill directory to the
//! same relative path below the destination.

use anyhow::Context;
use std::path::Path;

use super::{write_managed_bytes, FileInstallReport};

/// Copies each file below `source` to the same relative path below
/// `destination`.
///
/// Every file goes through the installer's managed write, thus the copy obeys
/// the same rule as the remainder of an install: it does not write over a
/// file that it did not write, unless `force` is set. The canonical
/// `SKILL.md` already holds its ownership stamp, thus a copy of its bytes
/// keeps that stamp.
pub fn copy_tree(
    source: &Path,
    destination: &Path,
    force: bool,
) -> anyhow::Result<Vec<FileInstallReport>> {
    let mut files = Vec::new();
    copy_directory(source, destination, force, &mut files)?;
    Ok(files)
}

fn copy_directory(
    source: &Path,
    destination: &Path,
    force: bool,
    files: &mut Vec<FileInstallReport>,
) -> anyhow::Result<()> {
    // Read the directory fully and sort it. The report then lists the files
    // in the same order on each platform.
    let mut entries = std::fs::read_dir(source)
        .with_context(|| format!("failed to read {}", source.display()))?
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&path, &target, force, files)?;
            continue;
        }
        let contents =
            std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        files.push(write_managed_bytes(&target, &contents, force)?);
    }
    Ok(())
}
