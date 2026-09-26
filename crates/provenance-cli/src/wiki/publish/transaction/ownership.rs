use super::{
    OutputIdentity, OutputState, PublicationLock, PublicationOutput, TransactionDirectory,
};
use crate::wiki::publish::PublishError;
use camino::Utf8Path;
use provenance_macros::rule;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub(in crate::wiki::publish) fn acquire_lock(
    transaction: &TransactionDirectory,
) -> Result<PublicationLock, PublishError> {
    let paths = &transaction.paths;
    let lock = transaction
        .create_file(&transaction.leaves.lock)
        .map_err(|error| PublishError::io("create publication lock", &paths.lock, error))?;
    let identity_file = lock
        .try_clone()
        .map_err(|error| PublishError::io("clone publication lock handle", &paths.lock, error))?;
    let identity = same_file::Handle::from_file(identity_file).map_err(|error| {
        PublishError::io("record publication lock identity", &paths.lock, error)
    })?;
    let mut lock = PublicationLock {
        file: lock,
        identity,
    };
    let initialization = lock
        .file
        .write_all(b"provenance wiki publication in progress\n")
        .map_err(|error| PublishError::io("write publication lock", &paths.lock, error))
        .and_then(|()| {
            lock.file
                .sync_all()
                .map_err(|error| PublishError::io("sync publication lock", &paths.lock, error))
        });
    if let Err(primary) = initialization {
        return match lock.cleanup(transaction) {
            Ok(()) => Err(primary),
            Err(cleanup) => Err(PublishError::CleanupFailed {
                primary: Box::new(primary),
                path: paths.lock.clone(),
                cleanup,
            }),
        };
    }
    Ok(lock)
}

/// Decides whether a publication may start against the path it was pointed at.
///
/// Publication replaces a whole directory tree, so it refuses to start from
/// any state it cannot reason about: a symlink at the output, which would let
/// a link redirect the replacement onto a tree the caller never named; a
/// non-directory at the output, which belongs to the caller; or any artifact
/// an interrupted earlier run left beside it (the lock, the lock cleanup, the
/// stage, the stage cleanup, or the backup).
///
/// Crash residue is reported by path and left exactly as found. Only an
/// operator can tell whether a leftover backup holds the live wiki or a
/// half-written one, so this code never adopts residue as its own work and
/// never deletes it to clear the way. The refusal is the whole response.
///
/// All five artifacts are probed before that refusal is built, so the operator
/// is told about everything an interrupted run left rather than about whichever
/// leftover happened to be looked at first. Clearing one artifact and retrying
/// only to be sent back for the next is the failure this avoids.
#[rule("rule_publish_preflight")]
pub(in crate::wiki::publish) fn preflight(
    output: &PublicationOutput,
    transaction: &TransactionDirectory,
) -> Result<OutputState, PublishError> {
    let paths = &transaction.paths;
    match child_kind(&transaction.parent, &transaction.output_leaf)
        .map_err(|error| PublishError::io("inspect wiki output", &output.path, error))?
    {
        Some(ChildKind::Symlink) => {
            return Err(PublishError::OutputSymlink {
                path: output.path.clone(),
            })
        }
        Some(ChildKind::File | ChildKind::Other) => {
            return Err(PublishError::OutputNotDirectory {
                path: output.path.clone(),
            })
        }
        _ => {}
    }
    let output_state = transaction.output_identity(&transaction.output_leaf, &output.path)?;
    transaction.validate_output(&transaction.output_leaf, &output.path, output.policy)?;

    let mut ambiguous = Vec::new();
    let mut unsafe_lock = None;
    for (leaf, path, is_lock) in [
        (&transaction.leaves.lock, &paths.lock, true),
        (&transaction.leaves.lock_cleanup, &paths.lock_cleanup, false),
        (&transaction.leaves.stage, &paths.stage, false),
        (
            &transaction.leaves.stage_cleanup,
            &paths.stage_cleanup,
            false,
        ),
        (&transaction.leaves.backup, &paths.backup, false),
    ] {
        let Some(kind) = child_kind(&transaction.parent, leaf)
            .map_err(|error| PublishError::io("inspect publication artifact", path, error))?
        else {
            continue;
        };
        if is_lock && kind != ChildKind::File {
            unsafe_lock = Some(path.clone());
        }
        ambiguous.push(path.clone());
    }
    if !ambiguous.is_empty() {
        return Err(PublishError::AmbiguousArtifacts {
            paths: ambiguous,
            unsafe_lock,
        });
    }
    Ok(output_state)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ChildKind {
    Directory,
    File,
    Symlink,
    Other,
}

#[cfg(unix)]
pub(super) fn child_kind(parent: &File, leaf: &str) -> std::io::Result<Option<ChildKind>> {
    let stat = match rustix::fs::statat(parent, leaf, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let kind = rustix::fs::FileType::from_raw_mode(stat.st_mode);
    Ok(Some(if kind.is_dir() {
        ChildKind::Directory
    } else if kind.is_file() {
        ChildKind::File
    } else if kind.is_symlink() {
        ChildKind::Symlink
    } else {
        ChildKind::Other
    }))
}

#[cfg(not(unix))]
pub(super) fn child_kind(parent: &File, leaf: &str) -> std::io::Result<Option<ChildKind>> {
    #[cfg(windows)]
    use std::os::windows::fs::MetadataExt;
    let mut options = fs_at::OpenOptions::default();
    options.follow(false);
    // open_path_at grants only SYNCHRONIZE by default and ignores read();
    // the metadata call below needs FILE_READ_ATTRIBUTES or Windows refuses
    // it with ERROR_ACCESS_DENIED.
    #[cfg(windows)]
    {
        use fs_at::os::windows::OpenOptionsExt;
        const FILE_READ_ATTRIBUTES: u32 = 0x0080;
        options.desired_access(FILE_READ_ATTRIBUTES);
    }
    let file = match options.open_path_at(parent, leaf) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    let file_type = metadata.file_type();
    #[cfg(windows)]
    let is_symlink = metadata.file_attributes() & 0x0000_0400 != 0;
    #[cfg(not(windows))]
    let is_symlink = file_type.is_symlink();
    Ok(Some(if is_symlink {
        ChildKind::Symlink
    } else if file_type.is_dir() {
        ChildKind::Directory
    } else if file_type.is_file() {
        ChildKind::File
    } else {
        ChildKind::Other
    }))
}

impl TransactionDirectory {
    pub(super) fn verify_requested_parent(&self, output: &Utf8Path) -> Result<(), PublishError> {
        let parent_path = output
            .parent()
            .filter(|path| !path.as_str().is_empty())
            .unwrap_or_else(|| Utf8Path::new("."));
        let expected =
            same_file::Handle::from_file(self.parent.try_clone().map_err(|error| {
                PublishError::io("clone output parent handle", parent_path, error)
            })?)
            .map_err(|error| {
                PublishError::io("record output parent identity", parent_path, error)
            })?;
        let current = open_directory_no_follow(parent_path.as_std_path())
            .and_then(same_file::Handle::from_file)
            .map_err(|error| PublishError::OutputChanged {
                path: output.to_path_buf(),
                detail: format!("output parent cannot be verified: {error}"),
            })?;
        if current == expected {
            Ok(())
        } else {
            Err(PublishError::OutputChanged {
                path: output.to_path_buf(),
                detail: "output parent no longer matches the opened transaction directory"
                    .to_string(),
            })
        }
    }

    pub(super) fn output_identity(
        &self,
        leaf: &str,
        display: &Utf8Path,
    ) -> Result<OutputState, PublishError> {
        match self.open_dir(leaf) {
            Ok(file) => same_file::Handle::from_file(file)
                .map(OutputIdentity)
                .map(OutputState::Existing)
                .map_err(|error| PublishError::io("open wiki output identity", display, error)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(OutputState::Absent),
            Err(error) => Err(PublishError::io(
                "inspect wiki output identity",
                display,
                error,
            )),
        }
    }
}

#[cfg(test)]
pub(super) fn output_identity(output: &Utf8Path) -> Result<OutputState, PublishError> {
    match output.symlink_metadata() {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(OutputState::Absent)
        }
        Err(error) => {
            return Err(PublishError::io(
                "inspect wiki output identity",
                output,
                error,
            ))
        }
    }
    let identity = same_file::Handle::from_path(output.as_std_path())
        .map_err(|error| PublishError::io("open wiki output identity", output, error))?;
    Ok(OutputState::Existing(OutputIdentity(identity)))
}

#[cfg(unix)]
pub(in crate::wiki::publish) fn open_child_directory_no_follow(
    parent: &File,
    leaf: &str,
) -> std::io::Result<File> {
    rustix::fs::openat(
        parent,
        leaf,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(std::io::Error::from)
}

#[cfg(windows)]
pub(in crate::wiki::publish) fn open_child_directory_no_follow(
    parent: &File,
    leaf: &str,
) -> std::io::Result<File> {
    use std::os::windows::fs::MetadataExt;

    let mut options = fs_at::OpenOptions::default();
    options.read(true).follow(false);
    let directory = options.open_dir_at(parent, leaf)?;
    if directory.metadata()?.file_attributes() & 0x0000_0400 != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "directory is a reparse point",
        ));
    }
    Ok(directory)
}

#[cfg(not(any(unix, windows)))]
pub(in crate::wiki::publish) fn open_child_directory_no_follow(
    parent: &File,
    leaf: &str,
) -> std::io::Result<File> {
    let mut options = fs_at::OpenOptions::default();
    options.follow(false);
    options.open_dir_at(parent, leaf)
}

#[cfg(unix)]
pub(in crate::wiki::publish) fn open_directory_no_follow(path: &Path) -> std::io::Result<File> {
    rustix::fs::openat(
        rustix::fs::CWD,
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(std::io::Error::from)
}

#[cfg(windows)]
pub(in crate::wiki::publish) fn open_directory_no_follow(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::MetadataExt;
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    let directory = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0x1 | 0x2 | 0x4)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    if directory.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "output parent is a reparse point",
        ));
    }
    Ok(directory)
}

#[cfg(not(any(unix, windows)))]
pub(in crate::wiki::publish) fn open_directory_no_follow(path: &Path) -> std::io::Result<File> {
    File::open(path)
}
