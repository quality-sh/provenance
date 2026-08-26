use anyhow::Context;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

mod commit;
mod rollback;
use commit::displace_to_backup;
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

    pub fn restore_if_owned(&self, path: &Path, owned: &Self) -> anyhow::Result<()> {
        owned.recheck(path)?;
        match self {
            Self::Missing => remove_if_owned(path, owned),
            Self::Regular { bytes, permissions } => {
                replace_contents(path, owned, bytes, Some(permissions.clone()), || {}).map(|_| ())
            }
        }
        .with_context(|| format!("failed to restore {}", path.display()))
    }
}

fn remove_if_owned(path: &Path, expected: &FileSnapshot) -> anyhow::Result<()> {
    remove_if_owned_with_hook(path, expected, || {})
}

fn remove_if_owned_with_hook(
    path: &Path,
    expected: &FileSnapshot,
    before_commit: impl FnOnce(),
) -> anyhow::Result<()> {
    expected.recheck(path)?;
    if matches!(expected, FileSnapshot::Missing) {
        return Ok(());
    }
    before_commit();
    let backup = displace_to_backup(path)
        .with_context(|| format!("{} changed during removal", path.display()))?;
    let displaced = FileSnapshot::read(&backup)?;
    if &displaced != expected {
        restore_displaced(&backup, path)?;
        anyhow::bail!("{} changed during removal", path.display());
    }
    if let Err(error) = commit::remove_preserved(&backup) {
        restore_displaced(&backup, path)?;
        return Err(error.into());
    }
    Ok(())
}

pub fn atomic_replace(path: &Path, before: &FileSnapshot, contents: &[u8]) -> anyhow::Result<()> {
    atomic_replace_with_hook(path, before, contents, || {}).map(|_| ())
}

fn atomic_replace_with_hook(
    path: &Path,
    before: &FileSnapshot,
    contents: &[u8],
    before_commit: impl FnOnce(),
) -> anyhow::Result<FileSnapshot> {
    let permissions = match before {
        FileSnapshot::Regular { permissions, .. } => Some(permissions.clone()),
        FileSnapshot::Missing => None,
    };
    replace_contents(path, before, contents, permissions, before_commit)
}

fn replace_contents(
    path: &Path,
    expected: &FileSnapshot,
    contents: &[u8],
    permissions: Option<std::fs::Permissions>,
    before_commit: impl FnOnce(),
) -> anyhow::Result<FileSnapshot> {
    expected.recheck(path)?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let temporary = temporary_path(parent, path)?;
    let prepared = (|| -> std::io::Result<FileSnapshot> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(contents)?;
        if let Some(permissions) = permissions {
            std::fs::set_permissions(&temporary, permissions)?;
        }
        file.sync_all()?;
        Ok(FileSnapshot::Regular {
            bytes: contents.to_vec(),
            permissions: file.metadata()?.permissions(),
        })
    })();
    if prepared.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    let prepared = prepared
        .with_context(|| format!("failed to prepare replacement for {}", path.display()))?;
    before_commit();
    commit_prepared(path, expected, &temporary)
        .with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(prepared)
}

fn commit_prepared(path: &Path, expected: &FileSnapshot, temporary: &Path) -> anyhow::Result<()> {
    let backup = match expected {
        FileSnapshot::Missing => None,
        FileSnapshot::Regular { .. } => {
            let backup = displace_to_backup(path)
                .with_context(|| format!("{} changed during replacement", path.display()))?;
            let displaced = FileSnapshot::read(&backup)?;
            if &displaced != expected {
                restore_displaced(&backup, path)?;
                let _ = std::fs::remove_file(temporary);
                anyhow::bail!("{} changed during replacement", path.display());
            }
            Some(backup)
        }
    };

    if let Err(error) = commit::rename_no_replace(temporary, path) {
        if let Some(backup) = &backup {
            restore_displaced(backup, path)?;
        }
        let _ = std::fs::remove_file(temporary);
        return Err(error).context(format!("{} changed during replacement", path.display()));
    }
    let _ = std::fs::remove_file(temporary);
    if let Some(backup) = backup {
        let _ = commit::remove_preserved(&backup);
    }
    Ok(())
}

fn restore_displaced(backup: &Path, path: &Path) -> anyhow::Result<()> {
    commit::rename_no_replace(backup, path).with_context(|| {
        format!(
            "could not restore concurrently changed {}; displaced bytes remain at {}",
            path.display(),
            backup.display()
        )
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

fn temporary_path(parent: &Path, path: &Path) -> std::io::Result<PathBuf> {
    unique_path(parent, path, "tmp")
}

fn unique_path(parent: &Path, path: &Path, kind: &str) -> std::io::Result<PathBuf> {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    for attempt in 0..100_u8 {
        let candidate = parent.join(format!(
            ".{name}.provenance-{}-{attempt}.{kind}",
            std::process::id(),
        ));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(std::io::Error::new(
        ErrorKind::AlreadyExists,
        format!("could not allocate {kind} file"),
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

    #[test]
    fn atomic_replace_does_not_overwrite_an_edit_created_during_commit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file.txt");
        std::fs::write(&path, "before\n").unwrap();
        let before = FileSnapshot::read(&path).unwrap();

        let error = atomic_replace_with_hook(&path, &before, b"planned\n", || {
            std::fs::write(&path, "concurrent\n").unwrap();
        })
        .unwrap_err();

        assert!(format!("{error:#}").contains("changed during replacement"));
        assert_eq!(std::fs::read(path).unwrap(), b"concurrent\n");
    }

    #[test]
    fn guarded_remove_does_not_delete_an_edit_created_during_commit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file.txt");
        std::fs::write(&path, "owned\n").unwrap();
        let owned = FileSnapshot::read(&path).unwrap();

        let error = remove_if_owned_with_hook(&path, &owned, || {
            std::fs::write(&path, "concurrent\n").unwrap();
        })
        .unwrap_err();

        assert!(format!("{error:#}").contains("changed during removal"));
        assert_eq!(std::fs::read(path).unwrap(), b"concurrent\n");
    }

    #[test]
    fn replacement_does_not_overwrite_a_colliding_backup_path() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file.txt");
        std::fs::write(&path, "before\n").unwrap();
        let before = FileSnapshot::read(&path).unwrap();
        let collision = directory.path().join(format!(
            ".file.txt.provenance-{}-0.backup",
            std::process::id()
        ));
        std::fs::write(&collision, "unrelated\n").unwrap();

        atomic_replace(&path, &before, b"planned\n").unwrap();

        assert_eq!(std::fs::read(path).unwrap(), b"planned\n");
        assert_eq!(std::fs::read(collision).unwrap(), b"unrelated\n");
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
