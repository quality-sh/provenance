//! Paths for file calls that do not accept long Windows paths themselves.

use camino::Utf8Path;
use std::path::PathBuf;

/// The form of `path` to give a file call outside `std::fs`, such as the
/// rename in `NamedTempFile::persist`.
///
/// `std::fs` adds the `\\?\` prefix to a long Windows path itself. Other
/// calls do not, and Windows refuses a path of 260 characters or more with
/// "The system cannot find the path specified". On Windows this function
/// returns the absolute path with that prefix. On other systems it returns
/// `path` unchanged.
#[cfg_attr(not(windows), allow(clippy::unnecessary_wraps))]
pub(crate) fn extended(path: &Utf8Path) -> std::io::Result<PathBuf> {
    #[cfg(windows)]
    {
        let absolute = std::path::absolute(path)?;
        let text = absolute.to_str().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "path is not UTF-8")
        })?;
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
        Ok(path.as_std_path().to_owned())
    }
}
