//! Held repository source files. Control storage remains a separate trusted input.
use camino::{Utf8Path, Utf8PathBuf};
use provenance_scanner::{FileScan, Language};
#[path = "files/native.rs"]
mod native;
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

pub(crate) struct OpenedRepositoryFile {
    pub relative: Utf8PathBuf,
    pub file: File,
}

pub(crate) struct RepositoryFiles {
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

/// Native paths resolve root aliases; source components remain lexical before the held open.
pub(crate) fn native_relative(
    root: &Utf8Path,
    path: &Utf8Path,
) -> Result<Utf8PathBuf, FileAccessRefusal> {
    native::relative(root, path)
}
