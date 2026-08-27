//! Inventories sidecar files carried beside a skill's `SKILL.md`.

use std::path::{Path, PathBuf};

pub(super) fn collect_sidecars(
    directory: &Path,
    relative: &Path,
    files: &mut Vec<(PathBuf, Vec<u8>)>,
) -> anyhow::Result<()> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let mut entries = entries.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let child = relative.join(entry.file_name());
        if child == Path::new("SKILL.md") {
            continue;
        }
        if entry.file_type()?.is_dir() {
            collect_sidecars(&entry.path(), &child, files)?;
        } else {
            files.push((child, std::fs::read(entry.path())?));
        }
    }
    Ok(())
}
