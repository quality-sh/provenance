use camino::Utf8PathBuf;
use std::fmt;
use std::io;

/// A publication failure whose variant states what the publisher proved.
#[derive(Debug)]
pub enum PublishError {
    InvalidOutputPath {
        path: Utf8PathBuf,
        detail: String,
    },
    OutputSymlink {
        path: Utf8PathBuf,
    },
    OutputNotDirectory {
        path: Utf8PathBuf,
    },
    CustomOutputUnrecognized {
        path: Utf8PathBuf,
    },
    InvalidManifest {
        path: Utf8PathBuf,
        detail: String,
    },
    UnknownManifestVersion {
        path: Utf8PathBuf,
        version: u32,
    },
    /// Every transaction artifact found beside the output, reported in one
    /// refusal so the operator sees the whole of what an interrupted run left.
    /// `unsafe_lock` names the lock when the entry standing in its place is not
    /// a regular file, which is a fact about one of the listed paths rather
    /// than a separate refusal.
    AmbiguousArtifacts {
        paths: Vec<Utf8PathBuf>,
        unsafe_lock: Option<Utf8PathBuf>,
    },
    InvalidRoute {
        route: String,
        detail: String,
    },
    OutputChanged {
        path: Utf8PathBuf,
        detail: String,
    },
    CleanupFailed {
        primary: Box<Self>,
        path: Utf8PathBuf,
        cleanup: io::Error,
    },
    Io {
        operation: &'static str,
        path: Utf8PathBuf,
        source: io::Error,
    },
    ReplacementRolledBack {
        output: Utf8PathBuf,
        source: io::Error,
    },
    RollbackFailed {
        output: Utf8PathBuf,
        backup: Utf8PathBuf,
        install: io::Error,
        rollback: io::Error,
    },
}

impl PublishError {
    pub(super) fn io(operation: &'static str, path: &camino::Utf8Path, source: io::Error) -> Self {
        Self::Io {
            operation,
            path: path.to_path_buf(),
            source,
        }
    }
}

impl fmt::Display for PublishError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOutputPath { path, detail } => {
                write!(formatter, "invalid wiki output path {path}: {detail}")
            }
            Self::OutputSymlink { path } => write!(
                formatter,
                "refusing wiki output symlink {path}; choose a real directory"
            ),
            Self::OutputNotDirectory { path } => write!(
                formatter,
                "refusing non-directory wiki output {path}; move it before publishing"
            ),
            Self::CustomOutputUnrecognized { path } => write!(
                formatter,
                "refusing nonempty custom wiki output {path}: it has no recognized {0} marker; move the directory, empty it, or use an output previously created by Provenance",
                super::OWNERSHIP_MANIFEST
            ),
            Self::InvalidManifest { path, detail } => {
                write!(formatter, "unrecognized wiki ownership manifest {path}: {detail}")
            }
            Self::UnknownManifestVersion { path, version } => write!(
                formatter,
                "unsupported wiki ownership manifest version {version} at {path}; upgrade Provenance or choose another output"
            ),
            Self::AmbiguousArtifacts { paths, unsafe_lock } => {
                write!(
                    formatter,
                    "wiki publication cannot safely continue because transaction artifacts are present: {}; inspect them and explicitly move or remove them before retrying",
                    paths.iter().map(|path| path.as_str()).collect::<Vec<_>>().join(", ")
                )?;
                unsafe_lock.as_ref().map_or(Ok(()), |path| write!(
                    formatter,
                    "; the entry at {path} is not a regular file, so something other than a publication lock is standing where the lock goes"
                ))
            }
            Self::InvalidRoute { route, detail } => {
                write!(formatter, "unsafe generated wiki route {route:?}: {detail}")
            }
            Self::OutputChanged { path, detail } => write!(
                formatter,
                "wiki output {path} changed during publication ({detail}); no replacement was attempted"
            ),
            Self::CleanupFailed {
                primary,
                path,
                cleanup,
            } => write!(
                formatter,
                "{primary}; cleanup of publisher-owned artifact {path} also failed: {cleanup}"
            ),
            Self::Io { operation, path, source } => {
                write!(formatter, "failed to {operation} {path}: {source}")
            }
            Self::ReplacementRolledBack { output, source } => write!(
                formatter,
                "failed to install the completed wiki at {output}; the previous output was restored: {source}"
            ),
            Self::RollbackFailed {
                output,
                backup,
                install,
                rollback,
            } => write!(
                formatter,
                "wiki installation at {output} failed ({install}) and rollback also failed ({rollback}); output ownership is ambiguous, so no cleanup was attempted; inspect backup {backup}"
            ),
        }
    }
}

impl std::error::Error for PublishError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } | Self::ReplacementRolledBack { source, .. } => Some(source),
            Self::CleanupFailed { primary, .. } => Some(primary),
            Self::RollbackFailed { install, .. } => Some(install),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
