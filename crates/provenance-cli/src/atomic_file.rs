use anyhow::Context;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileSnapshot {
    Missing,
    Regular(Vec<u8>),
}

impl FileSnapshot {
    pub fn read(path: &Path) -> anyhow::Result<Self> {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_file() => Ok(Self::Regular(std::fs::read(path)?)),
            Ok(_) => anyhow::bail!("{} is not a regular file", path.display()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Self::Missing),
            Err(error) => Err(error.into()),
        }
    }

    pub fn recheck(&self, path: &Path) -> anyhow::Result<()> {
        let current =
            Self::read(path).with_context(|| format!("failed to recheck {}", path.display()))?;
        anyhow::ensure!(
            &current == self,
            "{} changed after initialization was planned; retry",
            path.display()
        );
        Ok(())
    }

    pub fn bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Missing => None,
            Self::Regular(bytes) => Some(bytes),
        }
    }
}

pub fn atomic_replace(path: &Path, before: &FileSnapshot, contents: &[u8]) -> anyhow::Result<()> {
    before.recheck(path)?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let permissions = match before {
        FileSnapshot::Regular(_) => Some(std::fs::metadata(path)?.permissions()),
        FileSnapshot::Missing => None,
    };
    let temporary = temporary_path(parent, path)?;
    let result = (|| -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(contents)?;
        file.sync_all()?;
        if let Some(permissions) = permissions {
            std::fs::set_permissions(&temporary, permissions)?;
        }
        replace_path(&temporary, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.with_context(|| format!("failed to replace {}", path.display()))
}

#[cfg(not(windows))]
pub fn replace_path(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::rename(from, to)
}

#[cfg(windows)]
pub fn replace_path(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let from = from
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let to = to
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn temporary_path(parent: &Path, path: &Path) -> std::io::Result<PathBuf> {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    for attempt in 0..100_u8 {
        let candidate = parent.join(format!(
            ".{name}.provenance-{}-{attempt}.tmp",
            std::process::id()
        ));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(std::io::Error::new(
        ErrorKind::AlreadyExists,
        "could not allocate temporary file",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_an_absent_regular_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nested/file.txt");
        atomic_replace(&path, &FileSnapshot::Missing, b"planned\n").unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"planned\n");
    }

    #[test]
    fn refuses_a_stale_plan_without_changing_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file.txt");
        std::fs::write(&path, "before\n").unwrap();
        let before = FileSnapshot::read(&path).unwrap();
        std::fs::write(&path, "concurrent\n").unwrap();

        let error = atomic_replace(&path, &before, b"planned\n").unwrap_err();

        assert!(error.to_string().contains("changed after"));
        assert_eq!(std::fs::read(path).unwrap(), b"concurrent\n");
    }
}
