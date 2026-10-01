//! Held repository source files. Control storage remains a separate trusted input.
use camino::{Utf8Path, Utf8PathBuf};
use provenance_scanner::{FileScan, Language};
#[path = "files/native.rs"]
mod native;
#[path = "files/safe_fs.rs"]
mod safe_fs;
#[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
pub use safe_fs::NoReplaceUnsupported;
pub use safe_fs::{rename_no_replace, ChildKind, Directory};
#[cfg(any(unix, windows))]
#[path = "files/held.rs"]
mod held;
#[cfg(any(unix, windows))]
#[path = "files/held_io.rs"]
mod held_io;
#[cfg(any(unix, windows))]
#[path = "files/held_metadata.rs"]
mod held_metadata;
#[cfg(any(unix, windows))]
#[path = "files/held_recovery.rs"]
mod held_recovery;
#[cfg(any(unix, windows))]
pub use held::{
    BackupRetentionReason, FileIdentity, HeldRepositoryFile, PreparedRepositoryFile,
    RepositoryFileBackup, RepositoryFileInstall,
};
use std::{fs::File, io::Read};
#[cfg(test)]
#[path = "files/tests.rs"]
mod tests;
#[cfg(unix)]
#[path = "files/unix.rs"]
mod unix;
// The Linux test harness uses O_PATH, which is not available on macOS.
#[cfg(any(windows, all(test, target_os = "linux")))]
#[path = "files/windows.rs"]
mod windows;
#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

#[derive(Debug, thiserror::Error)]
pub enum FileAccessRefusal {
    #[error("repository file access denied")]
    Denied,
    #[error("repository file not found")]
    Missing,
    #[error("secure repository file access unavailable")]
    Unavailable,
    #[error("repository file read failed: {0}")]
    Read(#[source] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum RepositoryFileRefusal {
    #[error("repository file access denied")]
    Denied,
    #[error("repository file not found")]
    Missing,
    #[error("secure repository file access unavailable")]
    Unavailable,
    #[error("repository file read failed: {0}")]
    Read(#[source] std::io::Error),
    #[error("repository file exceeds the {limit}-byte limit")]
    TooLarge { limit: usize },
    #[error("repository file is not valid UTF-8")]
    InvalidUtf8,
    #[error("repository file changed after it was read")]
    Changed,
    #[error("repository file owner {owner} differs from effective user {effective}")]
    OwnerMismatch { owner: u32, effective: u32 },
    #[error("repository file write failed: {0}")]
    Write(#[source] std::io::Error),
    #[error("repository file restore failed: {0}")]
    Restore(#[source] std::io::Error),
}

impl From<FileAccessRefusal> for RepositoryFileRefusal {
    fn from(refusal: FileAccessRefusal) -> Self {
        match refusal {
            FileAccessRefusal::Denied => Self::Denied,
            FileAccessRefusal::Missing => Self::Missing,
            FileAccessRefusal::Unavailable => Self::Unavailable,
            FileAccessRefusal::Read(error) => Self::Read(error),
        }
    }
}

/// Read one regular UTF-8 repository file through held, no-follow traversal.
pub fn read_repository_file(
    root: &std::path::Path,
    relative: &std::path::Path,
) -> Result<Vec<u8>, RepositoryFileRefusal> {
    let root = Utf8Path::from_path(root).ok_or(RepositoryFileRefusal::Denied)?;
    let relative = Utf8Path::from_path(relative).ok_or(RepositoryFileRefusal::Denied)?;
    #[cfg(any(unix, windows))]
    {
        let files = RepositoryFiles::open(root).map_err(RepositoryFileRefusal::from)?;
        let held = files.read_bounded(relative, usize::MAX)?;
        Ok(held.bytes().to_vec())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (root, relative);
        Err(RepositoryFileRefusal::Unavailable)
    }
}

pub struct OpenedRepositoryFile {
    pub relative: Utf8PathBuf,
    pub file: File,
}

pub struct RepositoryFiles {
    path: Utf8PathBuf,
    #[cfg(any(unix, windows))]
    directory: File,
}
impl RepositoryFiles {
    pub fn open(path: &Utf8Path) -> Result<Self, FileAccessRefusal> {
        #[cfg(any(unix, windows))]
        {
            Ok(Self {
                path: path.to_owned(),
                directory: platform::open_root(path)?,
            })
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = path;
            Err(FileAccessRefusal::Unavailable)
        }
    }
    pub fn open_file(
        &self,
        relative: &Utf8Path,
    ) -> Result<OpenedRepositoryFile, FileAccessRefusal> {
        validate_relative(relative)?;
        #[cfg(any(unix, windows))]
        {
            Ok(OpenedRepositoryFile {
                relative: relative.to_owned(),
                file: platform::open_file(&self.directory, relative)?,
            })
        }
        #[cfg(not(any(unix, windows)))]
        {
            Err(FileAccessRefusal::Unavailable)
        }
    }
    pub fn scan_file(&self, relative: &Utf8Path) -> Result<Option<FileScan>, FileAccessRefusal> {
        validate_relative(relative)?;
        let Some(language) = relative.extension().and_then(Language::from_extension) else {
            return Ok(None);
        };
        let mut opened = match self.open_file(relative) {
            Ok(opened) => opened,
            Err(FileAccessRefusal::Missing) => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut content = String::new();
        opened
            .file
            .read_to_string(&mut content)
            .map_err(FileAccessRefusal::Read)?;
        Ok(Some(provenance_scanner::scan_file(
            &self.path.join(opened.relative),
            language,
            &content,
        )))
    }
    #[cfg(any(unix, windows))]
    pub fn read_bounded(
        &self,
        relative: &Utf8Path,
        limit: usize,
    ) -> Result<HeldRepositoryFile, RepositoryFileRefusal> {
        validate_relative(relative).map_err(RepositoryFileRefusal::from)?;
        held::open(&self.directory, &self.path, relative, limit)
    }
    pub fn scan_tree(&self, limit: usize) -> Result<(Vec<FileScan>, bool), FileAccessRefusal> {
        #[cfg(any(unix, windows))]
        {
            platform::scan_tree(&self.directory, &self.path, limit)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = limit;
            Err(FileAccessRefusal::Unavailable)
        }
    }
}

pub(crate) fn validate_relative(relative: &Utf8Path) -> Result<(), FileAccessRefusal> {
    if relative.as_str().contains(['\\', ':', '\0'])
        || relative
            .as_str()
            .split('/')
            .any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(FileAccessRefusal::Denied);
    }
    if relative
        .components()
        .any(|part| !matches!(part, camino::Utf8Component::Normal(_)))
    {
        return Err(FileAccessRefusal::Denied);
    }
    Ok(())
}

#[cfg(any(unix, windows))]
pub(crate) fn validate_recovery_artifact_leaf(
    relative: &Utf8Path,
    leaf: &str,
    suffix: &str,
) -> Result<(), RepositoryFileRefusal> {
    validate_relative(relative).map_err(RepositoryFileRefusal::from)?;
    let target = relative.file_name().ok_or(RepositoryFileRefusal::Changed)?;
    held::valid_artifact_leaf(target, leaf, suffix)
        .then_some(())
        .ok_or(RepositoryFileRefusal::Changed)
}

/// Native paths resolve root aliases; source components remain lexical before the held open.
pub(crate) fn native_relative(
    root: &Utf8Path,
    path: &Utf8Path,
) -> Result<Utf8PathBuf, FileAccessRefusal> {
    native::relative(root, path)
}
