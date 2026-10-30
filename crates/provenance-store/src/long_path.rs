//! Paths for file calls that do not accept long Windows paths themselves.

use std::path::{Path, PathBuf};

/// The form of `path` to give a file call outside `std::fs`, such as the
/// rename in `NamedTempFile::persist` or `MoveFileExW`.
///
/// `std::fs` adds the `\\?\` prefix to a long Windows path itself. Other
/// calls do not, and Windows refuses a path of 260 characters or more with
/// "The system cannot find the path specified". On Windows this function
/// returns the absolute path with that prefix, and an empty path is the
/// current directory, as it is for `tempfile`. A path that is not UTF-8 stays
/// absolute without the prefix. On other systems it returns `path` unchanged.
#[cfg_attr(not(windows), allow(clippy::unnecessary_wraps))]
pub fn extended(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    let path = path.as_ref();
    #[cfg(windows)]
    {
        let path = if path.as_os_str().is_empty() {
            Path::new(".")
        } else {
            path
        };
        let absolute = std::path::absolute(path)?;
        let Some(text) = absolute.to_str() else {
            return Ok(absolute);
        };
        Ok(PathBuf::from(if text.starts_with(r"\\?\") {
            text.to_owned()
        } else if let Some(device) = text.strip_prefix(r"\\.\") {
            format!(r"\\?\{device}")
        } else if let Some(share) = text.strip_prefix(r"\\") {
            format!(r"\\?\UNC\{share}")
        } else {
            format!(r"\\?\{text}")
        }))
    }
    #[cfg(not(windows))]
    {
        Ok(path.to_owned())
    }
}

