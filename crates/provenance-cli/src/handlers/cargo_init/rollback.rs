use crate::atomic_file::FileSnapshot;
use anyhow::Context;
use std::path::{Path, PathBuf};

pub(super) struct CargoPaths {
    pub manifest: PathBuf,
    pub lock: PathBuf,
}

pub(super) struct CargoRollback {
    paths: CargoPaths,
    before: CargoFilesState,
    after: Option<CargoFilesState>,
}

impl CargoRollback {
    pub(super) fn capture(paths: CargoPaths) -> anyhow::Result<Self> {
        let before = CargoFilesState::capture(&paths).context("failed to snapshot Cargo files")?;
        Ok(Self {
            paths,
            before,
            after: None,
        })
    }

    pub(super) fn observe_after(mut self) -> anyhow::Result<Self> {
        let manifest = FileSnapshot::read(&self.paths.manifest)
            .context("failed to observe Cargo.toml after `cargo add`")?;
        let lock = match FileSnapshot::read(&self.paths.lock) {
            Ok(lock) => lock,
            Err(error) => {
                return match self
                    .before
                    .manifest
                    .restore_if_owned(&self.paths.manifest, &manifest)
                {
                    Ok(()) => Err(error)
                        .context("restored Cargo.toml after post-command observation failed"),
                    Err(rollback) => Err(error).context(format!(
                        "post-command observation failed and Cargo.toml rollback failed: {rollback:#}"
                    )),
                };
            }
        };
        self.after = Some(CargoFilesState { manifest, lock });
        Ok(self)
    }

    #[provenance_macros::rule("rule_cargo_init_restores_owned_files")]
    pub(super) fn rollback(self) -> anyhow::Result<()> {
        let after = self
            .after
            .context("Cargo rollback has no observed post-command state")?;
        let current = CargoFilesState::capture(&self.paths)
            .context("failed to inspect Cargo files before rollback")?;
        anyhow::ensure!(
            current == after,
            "Cargo.toml or Cargo.lock changed after `cargo add`; neither file was restored"
        );

        restore_cargo_pair(
            || {
                self.before
                    .manifest
                    .restore_if_owned(&self.paths.manifest, &after.manifest)
            },
            || {
                self.before
                    .lock
                    .restore_if_owned(&self.paths.lock, &after.lock)
            },
            || {
                after
                    .manifest
                    .restore_if_owned(&self.paths.manifest, &self.before.manifest)
            },
            &self.paths.manifest,
            &self.paths.lock,
        )
    }
}

#[derive(PartialEq, Eq)]
struct CargoFilesState {
    manifest: FileSnapshot,
    lock: FileSnapshot,
}

impl CargoFilesState {
    fn capture(paths: &CargoPaths) -> anyhow::Result<Self> {
        Ok(Self {
            manifest: FileSnapshot::read(&paths.manifest)?,
            lock: FileSnapshot::read(&paths.lock)?,
        })
    }
}

fn restore_cargo_pair(
    restore_manifest: impl FnOnce() -> anyhow::Result<()>,
    restore_lock: impl FnOnce() -> anyhow::Result<()>,
    compensate_manifest: impl FnOnce() -> anyhow::Result<()>,
    manifest_path: &Path,
    lock_path: &Path,
) -> anyhow::Result<()> {
    restore_manifest().with_context(|| format!("failed to restore {}", manifest_path.display()))?;
    let Err(error) = restore_lock() else {
        return Ok(());
    };
    match compensate_manifest() {
        Ok(()) => Err(error).with_context(|| {
            format!(
                "failed to restore {}; restored {} to its post-Cargo state",
                lock_path.display(),
                manifest_path.display()
            )
        }),
        Err(compensation) => Err(error).with_context(|| {
            format!(
                "failed to restore {}; also failed to return {} to its post-Cargo state: {compensation}",
                lock_path.display(),
                manifest_path.display()
            )
        }),
    }
}

#[cfg(test)]
#[path = "rollback_tests.rs"]
mod tests;
