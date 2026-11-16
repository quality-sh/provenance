use anyhow::Context;
use provenance_store::publication::{
    remove_synced_file_checked, write_synced_file_checked, ChangedUnsynced,
};
use std::io::ErrorKind;
use std::path::Path;

mod commit;
mod rollback;
pub use rollback::FileRollbackJournal;

#[provenance_macros::rule("rule_init_managed_paths_stay_in_repository")]
pub fn ensure_managed_directory(root: &Path, directory: &Path) -> anyhow::Result<()> {
    let relative = directory.strip_prefix(root).with_context(|| {
        format!(
            "managed path {} is outside {}",
            directory.display(),
            root.display()
        )
    })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                anyhow::ensure!(
                    !metadata.file_type().is_symlink(),
                    "managed path contains a symlink component: {}",
                    current.display()
                );
                anyhow::ensure!(
                    metadata.is_dir(),
                    "managed path contains a non-directory component: {}",
                    current.display()
                );
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub enum FileSnapshot {
    Missing,
    Regular {
        bytes: Vec<u8>,
        permissions: std::fs::Permissions,
    },
}

impl PartialEq for FileSnapshot {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Missing, Self::Missing) => true,
            (
                Self::Regular {
                    bytes: left_bytes,
                    permissions: left_permissions,
                },
                Self::Regular {
                    bytes: right_bytes,
                    permissions: right_permissions,
                },
            ) => {
                left_bytes == right_bytes && permissions_equal(left_permissions, right_permissions)
            }
            _ => false,
        }
    }
}

impl Eq for FileSnapshot {}

impl FileSnapshot {
    pub fn read(path: &Path) -> anyhow::Result<Self> {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_file() => Ok(Self::Regular {
                bytes: std::fs::read(path)?,
                permissions: metadata.permissions(),
            }),
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
            Self::Regular { bytes, .. } => Some(bytes),
        }
    }

    const fn permissions(&self) -> Option<&std::fs::Permissions> {
        match self {
            Self::Missing => None,
            Self::Regular { permissions, .. } => Some(permissions),
        }
    }

    /// Puts this state back at `path` when `path` still holds `owned`. An
    /// error after the file changed tells that the file was restored and
    /// that its folder was not synced.
    pub fn restore_if_owned(&self, path: &Path, owned: &Self) -> anyhow::Result<()> {
        owned.recheck(path)?;
        let synced = match self {
            Self::Missing => remove_if_owned(path, owned),
            Self::Regular { bytes, permissions } => {
                replace_file(path, owned, bytes, Some(permissions)).map(|replaced| replaced.synced)
            }
        }
        .with_context(|| format!("failed to restore {}", path.display()))?;
        synced.map_err(|unsynced| unsynced_error(path, "restored", unsynced))
    }
}

/// A project file that init changed: the state that init wrote, and the
/// sync of its folder after the change.
#[derive(Debug)]
struct Changed {
    after: FileSnapshot,
    synced: Result<(), ChangedUnsynced>,
}

/// The error of a file that init `changed` and whose folder it did not sync.
fn unsynced_error(path: &Path, changed: &str, unsynced: ChangedUnsynced) -> anyhow::Error {
    anyhow::Error::new(unsynced).context(format!(
        "{} was {changed}, but its folder was not synced",
        path.display()
    ))
}

/// Removes the file at `path`, which init planned from `expected`, after a
/// compare of `path` with `expected`, with one remove and a sync of its
/// folder.
fn remove_if_owned(
    path: &Path,
    expected: &FileSnapshot,
) -> anyhow::Result<Result<(), ChangedUnsynced>> {
    expected.recheck(path)?;
    if matches!(expected, FileSnapshot::Missing) {
        return Ok(Ok(()));
    }
    let removed = remove_synced_file_checked(path, || {
        crate::test_probes::at("project_file_removal_checked");
        anyhow::ensure!(
            &FileSnapshot::read(path)? == expected,
            "{} changed during removal",
            path.display()
        );
        crate::test_probes::at("project_file_removal_compared");
        Ok(())
    })?;
    crate::test_probes::at("project_file_removed");
    Ok(removed.map(|_removed| ()))
}

/// Replaces the file at `path`, which init planned from `expected`, with
/// `contents` in one rename, after a compare of `path` with `expected`.
#[provenance_macros::rule("rule_init_stop_keeps_each_project_file_whole")]
#[provenance_macros::rule("rule_init_refuses_a_project_file_change_before_its_compare")]
fn replace_file(
    path: &Path,
    expected: &FileSnapshot,
    contents: &[u8],
    permissions: Option<&std::fs::Permissions>,
) -> anyhow::Result<Changed> {
    expected.recheck(path)?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let prefix = format!(".{name}.provenance-");
    let replaced = write_synced_file_checked(path, contents, &prefix, permissions, || {
        crate::test_probes::at("project_file_prepared");
        anyhow::ensure!(
            &FileSnapshot::read(path)? == expected,
            "{} changed during replacement",
            path.display()
        );
        crate::test_probes::at("project_file_compared");
        Ok(())
    })
    .with_context(|| format!("failed to replace {}", path.display()))?;
    crate::test_probes::at("project_file_replaced");
    let (permissions, synced) = match replaced {
        Ok(permissions) => (permissions, Ok(())),
        Err((permissions, unsynced)) => (permissions, Err(unsynced)),
    };
    Ok(Changed {
        after: FileSnapshot::Regular {
            bytes: contents.to_vec(),
            permissions,
        },
        synced,
    })
}

fn restore_displaced(backup: &Path, path: &Path) -> anyhow::Result<()> {
    provenance_store::operations::files::rename_no_replace(backup, path).map_err(|error| {
        if error.kind() == ErrorKind::Unsupported {
            anyhow::Error::new(error).context(format!(
                "failed to restore {} from {}",
                path.display(),
                backup.display()
            ))
        } else {
            anyhow::Error::new(error).context(format!(
                "could not restore concurrently changed {}; displaced bytes remain at {}",
                path.display(),
                backup.display()
            ))
        }
    })
}

#[cfg(unix)]
fn permissions_equal(left: &std::fs::Permissions, right: &std::fs::Permissions) -> bool {
    use std::os::unix::fs::PermissionsExt;
    left.mode() == right.mode()
}

#[cfg(not(unix))]
fn permissions_equal(left: &std::fs::Permissions, right: &std::fs::Permissions) -> bool {
    left.readonly() == right.readonly()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_an_absent_regular_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nested/file.txt");
        replace_file(&path, &FileSnapshot::Missing, b"planned\n", None).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"planned\n");
    }

    #[test]
    fn refuses_a_stale_plan_without_changing_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file.txt");
        std::fs::write(&path, "before\n").unwrap();
        let before = FileSnapshot::read(&path).unwrap();
        std::fs::write(&path, "concurrent\n").unwrap();

        let error = replace_file(&path, &before, b"planned\n", None).unwrap_err();

        assert!(error.to_string().contains("changed after"));
        assert_eq!(std::fs::read(path).unwrap(), b"concurrent\n");
    }

    #[cfg(unix)]
    #[test]
    fn snapshots_include_permissions_in_file_ownership() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file.txt");
        std::fs::write(&path, "same bytes\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        let before = FileSnapshot::read(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

        assert_ne!(FileSnapshot::read(&path).unwrap(), before);
    }

    #[cfg(unix)]
    #[test]
    fn guarded_restore_returns_owned_bytes_and_permissions() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file.txt");
        std::fs::write(&path, "before\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
        let before = FileSnapshot::read(&path).unwrap();
        std::fs::write(&path, "owned\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let owned = FileSnapshot::read(&path).unwrap();

        before.restore_if_owned(&path, &owned).unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"before\n");
        assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o640);
    }

    #[test]
    fn guarded_restore_refuses_to_overwrite_a_concurrent_edit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file.txt");
        std::fs::write(&path, "before\n").unwrap();
        let before = FileSnapshot::read(&path).unwrap();
        std::fs::write(&path, "owned\n").unwrap();
        let owned = FileSnapshot::read(&path).unwrap();
        std::fs::write(&path, "concurrent\n").unwrap();

        let error = before.restore_if_owned(&path, &owned).unwrap_err();

        assert!(error.to_string().contains("changed after"));
        assert_eq!(std::fs::read(&path).unwrap(), b"concurrent\n");
    }

    #[test]
    #[provenance_macros::verifies("rule_init_apply_rolls_back_owned_changes", examples)]
    fn journal_rollback_preserves_concurrent_edits_and_restores_other_changes() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first.txt");
        let second = directory.path().join("second.txt");
        std::fs::write(&first, "first before\n").unwrap();
        std::fs::write(&second, "second before\n").unwrap();
        let first_before = FileSnapshot::read(&first).unwrap();
        let second_before = FileSnapshot::read(&second).unwrap();
        let mut journal = FileRollbackJournal::default();
        journal
            .replace(&first, &first_before, b"first owned\n")
            .unwrap();
        journal
            .replace(&second, &second_before, b"second owned\n")
            .unwrap();
        std::fs::write(&first, "first concurrent\n").unwrap();

        let error = journal.rollback().unwrap_err();

        assert!(format!("{error:#}").contains("first.txt"));
        assert_eq!(
            std::fs::read_to_string(first).unwrap(),
            "first concurrent\n"
        );
        assert_eq!(std::fs::read_to_string(second).unwrap(), "second before\n");
    }
}
