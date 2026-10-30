//! Paths for file calls that do not accept long Windows paths themselves.

use provenance_macros::rule;
use std::path::{Path, PathBuf};

/// The form of `path` to give a file call outside `std::fs`, such as the
/// rename in `NamedTempFile::persist`. On Windows it is the absolute path with
/// the `\\?\` prefix, and an empty path is the current directory; a path that
/// is not UTF-8 stays absolute without the prefix. On other systems it is
/// `path` unchanged.
#[rule("rule_windows_long_path_writes")]
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

#[cfg(all(test, windows))]
mod tests {
    use super::extended;
    use std::path::PathBuf;

    #[test]
    fn extended_gives_the_prefixed_absolute_form() {
        let current = std::env::current_dir().unwrap();
        let current = current.to_str().unwrap();
        let cases = [
            (r"C:\repo\file", r"\\?\C:\repo\file".to_owned()),
            (
                r"\\server\share\file",
                r"\\?\UNC\server\share\file".to_owned(),
            ),
            (r"\\.\C:\repo\file", r"\\?\C:\repo\file".to_owned()),
            (r"\\?\C:\repo\file", r"\\?\C:\repo\file".to_owned()),
            ("file", format!(r"\\?\{current}\file")),
            ("", format!(r"\\?\{current}")),
        ];
        for (input, expected) in cases {
            assert_eq!(
                extended(input).unwrap(),
                PathBuf::from(expected),
                "{input:?}"
            );
        }
    }
}
