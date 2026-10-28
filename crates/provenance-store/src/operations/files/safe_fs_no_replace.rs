use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct NoReplaceUnsupported {
    operation: &'static str,
    path: PathBuf,
    source: std::io::Error,
}

impl std::fmt::Display for NoReplaceUnsupported {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "filesystem does not support atomic no-replace installation: {} failed for {}: {}",
            self.operation,
            self.path.display(),
            self.source
        )
    }
}

impl std::error::Error for NoReplaceUnsupported {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

pub(super) fn with_fallback(
    from: &Path,
    to: &Path,
    rename: impl FnOnce() -> std::io::Result<()>,
    link: impl FnOnce() -> std::io::Result<()>,
    unlink: impl FnOnce() -> std::io::Result<()>,
) -> std::io::Result<()> {
    match rename() {
        Ok(()) => return Ok(()),
        Err(error) if rename_is_unsupported(&error) => {}
        Err(error) => return Err(error),
    }
    match link() {
        Ok(()) => unlink().map_err(|error| {
            std::io::Error::new(
                error.kind(),
                format!("unlink failed for {}: {error}", from.display()),
            )
        }),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Err(error),
        Err(source) => Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            NoReplaceUnsupported {
                operation: "link",
                path: to.to_path_buf(),
                source,
            },
        )),
    }
}

fn rename_is_unsupported(error: &std::io::Error) -> bool {
    let code = error.raw_os_error();
    [
        rustix::io::Errno::INVAL,
        rustix::io::Errno::NOSYS,
        rustix::io::Errno::NOTSUP,
        rustix::io::Errno::OPNOTSUPP,
    ]
    .into_iter()
    .any(|unsupported| code == Some(unsupported.raw_os_error()))
}
