use super::{
    held::{self, valid_artifact_leaf, HeldRepositoryFile},
    held_io::read,
    held_metadata::FileIdentity,
    platform, safe_fs, validate_relative, RepositoryFileRefusal as Refusal, RepositoryFiles,
    Utf8Path,
};
use crate::storage::local::durability::sync_opened_directory;
use sha2::{Digest as _, Sha256};
use std::io::Read as _;

#[derive(Clone, Debug)]
pub struct RepositoryFileBackup {
    leaf: String,
    identity: FileIdentity,
    digest: [u8; 32],
}

impl RepositoryFileBackup {
    pub(super) const fn new(leaf: String, identity: FileIdentity, digest: [u8; 32]) -> Self {
        Self {
            leaf,
            identity,
            digest,
        }
    }

    pub(crate) fn from_recovery(
        leaf: String,
        identity: FileIdentity,
        digest: [u8; 32],
    ) -> Result<Self, Refusal> {
        if matches!(leaf.as_str(), "" | "." | "..") || leaf.contains(['/', '\\', ':', '\0']) {
            return Err(Refusal::Changed);
        }
        Ok(Self {
            leaf,
            identity,
            digest,
        })
    }

    pub fn leaf(&self) -> &str {
        &self.leaf
    }

    pub const fn identity(&self) -> &FileIdentity {
        &self.identity
    }

    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

impl RepositoryFiles {
    pub(crate) fn restore_recovery_backup(
        &self,
        relative: &Utf8Path,
        backup_leaf: &str,
        expected_identity: &FileIdentity,
        expected_digest: [u8; 32],
    ) -> Result<HeldRepositoryFile, Refusal> {
        validate_relative(relative).map_err(Refusal::from)?;
        let (parent, target_leaf) =
            platform::open_parent(&self.directory, relative).map_err(Refusal::from)?;
        if !valid_artifact_leaf(&target_leaf, backup_leaf, ".backup") {
            return Err(Refusal::Changed);
        }
        require_missing(&parent, &target_leaf)?;
        let backup = platform::regular(&parent, backup_leaf).map_err(Refusal::from)?;
        require_matching_artifact(backup, expected_identity, expected_digest)?;
        safe_fs::rename_no_replace_in(&parent, backup_leaf, &target_leaf)
            .map_err(Refusal::Restore)?;
        held::open(&self.directory, &self.path, relative, usize::MAX)
    }

    /// Removes the prepared file `leaf` next to the file at `relative` when
    /// it is the store's own data: its bytes are `replacement` or a leading
    /// part of it. No file there is a success. A file with other content, or
    /// any file when `replacement` is `None`, is a change that a person made:
    /// the call refuses it with `Changed`, and the file stays.
    pub(crate) fn remove_own_prepared(
        &self,
        relative: &Utf8Path,
        leaf: &str,
        replacement: Option<&[u8]>,
    ) -> Result<(), Refusal> {
        validate_relative(relative).map_err(Refusal::from)?;
        let (parent, target_leaf) =
            platform::open_parent(&self.directory, relative).map_err(Refusal::from)?;
        remove_own_prepared(&parent, &target_leaf, leaf, replacement)
    }
}

impl HeldRepositoryFile {
    /// Removes the prepared file `leaf` next to this file when it is the
    /// store's own data, as [`RepositoryFiles::remove_own_prepared`] does.
    pub(crate) fn remove_own_prepared(
        &self,
        leaf: &str,
        replacement: &[u8],
    ) -> Result<(), Refusal> {
        remove_own_prepared(self.parent(), self.leaf(), leaf, Some(replacement))
    }

    /// Completes the install of the replacement that this file holds: removes
    /// `backup` when it holds the old file, and then syncs the directory of
    /// this file. A backup with other bytes stays, as the swap keeps it.
    pub(crate) fn finish_recovered_install(
        &self,
        backup: &RepositoryFileBackup,
    ) -> Result<(), Refusal> {
        if !valid_artifact_leaf(self.leaf(), backup.leaf(), ".backup") {
            return Err(Refusal::Changed);
        }
        if let Some(file) = optional_artifact(self.parent(), backup.leaf())? {
            if require_matching_artifact(file, backup.identity(), backup.digest()).is_ok() {
                unlink(self.parent(), backup.leaf())?;
            }
        }
        sync_opened_directory(self.parent()).map_err(Refusal::Write)
    }
}

fn remove_own_prepared(
    parent: &std::fs::File,
    target_leaf: &str,
    leaf: &str,
    replacement: Option<&[u8]>,
) -> Result<(), Refusal> {
    if !valid_artifact_leaf(target_leaf, leaf, ".tmp") {
        return Err(Refusal::Changed);
    }
    let Some(file) = optional_artifact(parent, leaf)? else {
        return Ok(());
    };
    // A file longer than the replacement is not a leading part of it, so the
    // read stops one byte past that length. The bytes need not be UTF-8: a
    // stop can cut a character.
    let limit = replacement.map_or(0, <[u8]>::len);
    let mut bytes = Vec::new();
    std::io::Read::take(file, u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(Refusal::Read)?;
    if !replacement.is_some_and(|replacement| replacement.starts_with(&bytes)) {
        return Err(Refusal::Changed);
    }
    unlink(parent, leaf)
}

fn unlink(parent: &std::fs::File, leaf: &str) -> Result<(), Refusal> {
    fs_at::OpenOptions::default()
        .unlink_at(parent, leaf)
        .map_err(Refusal::Write)
}

fn require_missing(parent: &std::fs::File, leaf: &str) -> Result<(), Refusal> {
    match platform::regular(parent, leaf).map_err(Refusal::from) {
        Err(Refusal::Missing) => Ok(()),
        Err(error) => Err(error),
        Ok(_) => Err(Refusal::Changed),
    }
}

fn optional_artifact(parent: &std::fs::File, leaf: &str) -> Result<Option<std::fs::File>, Refusal> {
    match platform::regular(parent, leaf).map_err(Refusal::from) {
        Err(Refusal::Missing) => Ok(None),
        Err(error) => Err(error),
        Ok(file) => Ok(Some(file)),
    }
}

fn require_matching_artifact(
    file: std::fs::File,
    expected_identity: &FileIdentity,
    expected_digest: [u8; 32],
) -> Result<(), Refusal> {
    let (bytes, found_identity, _) = read(file, usize::MAX)?;
    if &found_identity != expected_identity {
        return Err(Refusal::Changed);
    }
    if <[u8; 32]>::from(Sha256::digest(bytes)) != expected_digest {
        return Err(Refusal::Changed);
    }
    Ok(())
}
