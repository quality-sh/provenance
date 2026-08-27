use super::{ErrorKind, FileSnapshot};
use anyhow::Context;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct FileRollbackJournal {
    changes: Vec<FileChange>,
    managed_root: Option<PathBuf>,
}

enum FileChange {
    File {
        path: PathBuf,
        before: FileSnapshot,
        after: FileSnapshot,
    },
    CreatedSymlink {
        path: PathBuf,
        target: PathBuf,
    },
    Displaced {
        path: PathBuf,
        backup: PathBuf,
    },
    CreatedDirectory(PathBuf),
}

impl FileRollbackJournal {
    pub fn within(managed_root: &Path) -> Self {
        Self {
            changes: Vec::new(),
            managed_root: Some(managed_root.to_path_buf()),
        }
    }

    pub fn replace(
        &mut self,
        path: &Path,
        before: &FileSnapshot,
        contents: &[u8],
    ) -> anyhow::Result<()> {
        self.create_dir_all(path.parent().unwrap_or_else(|| Path::new(".")))?;
        let after = super::atomic_replace_with_hook(path, before, contents, || {})?;
        self.changes.push(FileChange::File {
            path: path.to_path_buf(),
            before: before.clone(),
            after,
        });
        Ok(())
    }

    pub fn remove(&mut self, path: &Path, before: &FileSnapshot) -> anyhow::Result<()> {
        super::remove_if_owned(path, before)?;
        let after = FileSnapshot::Missing;
        self.changes.push(FileChange::File {
            path: path.to_path_buf(),
            before: before.clone(),
            after,
        });
        Ok(())
    }

    pub fn displace_checked(
        &mut self,
        path: &Path,
        verify: impl FnOnce(&Path) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        if let Some(root) = &self.managed_root {
            super::ensure_managed_directory(root, path.parent().unwrap_or_else(|| Path::new(".")))?;
        }
        let backup = super::commit::displace_to_backup(path)
            .with_context(|| format!("failed to preserve {} before replacement", path.display()))?;
        if let Err(error) = verify(&backup) {
            super::commit::rename_no_replace(&backup, path).with_context(|| {
                format!(
                    "{} changed during displacement and could not be restored from {}",
                    path.display(),
                    backup.display()
                )
            })?;
            return Err(error.context(format!("{} changed during displacement", path.display())));
        }
        self.changes.push(FileChange::Displaced {
            path: path.to_path_buf(),
            backup,
        });
        Ok(())
    }

    pub fn create_dir_all(&mut self, path: &Path) -> anyhow::Result<()> {
        if let Some(root) = &self.managed_root {
            super::ensure_managed_directory(root, path)?;
        }
        let mut missing = Vec::new();
        let mut current = path;
        loop {
            if self.managed_root.as_deref() == Some(current) {
                match std::fs::metadata(current) {
                    Ok(metadata) => anyhow::ensure!(
                        metadata.is_dir(),
                        "managed repository root is not a directory: {}",
                        current.display()
                    ),
                    Err(error) if error.kind() == ErrorKind::NotFound => {
                        std::fs::create_dir_all(current)?;
                        self.changes
                            .push(FileChange::CreatedDirectory(current.to_path_buf()));
                    }
                    Err(error) => return Err(error.into()),
                }
                break;
            }
            match std::fs::symlink_metadata(current) {
                Ok(metadata) => {
                    anyhow::ensure!(
                        metadata.is_dir() && !metadata.file_type().is_symlink(),
                        "managed path contains a symlink or non-directory component: {}",
                        current.display()
                    );
                    break;
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    missing.push(current.to_path_buf());
                    current = current
                        .parent()
                        .ok_or_else(|| anyhow::anyhow!("managed path has no existing ancestor"))?;
                    if let Some(root) = &self.managed_root {
                        anyhow::ensure!(
                            current.starts_with(root),
                            "managed path {} is outside {}",
                            path.display(),
                            root.display()
                        );
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
        for directory in missing.into_iter().rev() {
            std::fs::create_dir(&directory)?;
            self.changes.push(FileChange::CreatedDirectory(directory));
        }
        Ok(())
    }

    pub fn record_created_symlink(&mut self, path: &Path, target: &Path) {
        self.changes.push(FileChange::CreatedSymlink {
            path: path.to_path_buf(),
            target: target.to_path_buf(),
        });
    }

    pub fn rollback(self) -> anyhow::Result<()> {
        let mut failures = Vec::new();
        for change in self.changes.into_iter().rev() {
            let (path, result) = match change {
                FileChange::File {
                    path,
                    before,
                    after,
                } => {
                    let result = before.restore_if_owned(&path, &after);
                    (path, result)
                }
                FileChange::CreatedSymlink { path, target } => {
                    let result = rollback_created_symlink(&path, &target);
                    (path, result)
                }
                FileChange::CreatedDirectory(path) => {
                    let result = std::fs::remove_dir(&path).map_err(Into::into);
                    (path, result)
                }
                FileChange::Displaced { path, backup } => {
                    let result = super::commit::rename_no_replace(&backup, &path)
                        .with_context(|| format!("failed to restore {}", path.display()));
                    (path, result)
                }
            };
            if let Err(error) = result {
                failures.push(format!("{}: {error:#}", path.display()));
            }
        }
        anyhow::ensure!(
            failures.is_empty(),
            "initialization rollback failure(s):\n- {}",
            failures.join("\n- ")
        );
        Ok(())
    }

    pub fn commit(self) -> anyhow::Result<()> {
        let mut failures = Vec::new();
        for change in self.changes {
            let FileChange::Displaced { backup, .. } = change else {
                continue;
            };
            let result = match super::commit::remove_preserved(&backup) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            };
            if let Err(error) = result {
                failures.push(format!("{}: {error}", backup.display()));
            }
        }
        anyhow::ensure!(
            failures.is_empty(),
            "initialization backup cleanup failure(s):\n- {}",
            failures.join("\n- ")
        );
        Ok(())
    }
}

fn rollback_created_symlink(path: &Path, target: &Path) -> anyhow::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    anyhow::ensure!(
        metadata.file_type().is_symlink() && std::fs::read_link(path)? == target,
        "symlink changed after initialization wrote it"
    );
    #[cfg(unix)]
    std::fs::remove_file(path)?;
    #[cfg(windows)]
    std::fs::remove_dir(path)?;
    Ok(())
}
