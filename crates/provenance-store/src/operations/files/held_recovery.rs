use super::{
    held::{self, valid_artifact_leaf, HeldRepositoryFile},
    held_io::read,
    held_metadata::FileIdentity,
    platform, safe_fs, validate_relative, RepositoryFileRefusal as Refusal, RepositoryFiles,
    Utf8Path,
};
use sha2::{Digest as _, Sha256};

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
        match platform::regular(&parent, &target_leaf).map_err(Refusal::from) {
            Err(Refusal::Missing) => {}
            Err(error) => return Err(error),
            Ok(_) => return Err(Refusal::Changed),
        }
        let backup = platform::regular(&parent, backup_leaf).map_err(Refusal::from)?;
        let (bytes, found_identity, _) = read(backup, usize::MAX)?;
        if &found_identity != expected_identity
            || <[u8; 32]>::from(Sha256::digest(bytes)) != expected_digest
        {
            return Err(Refusal::Changed);
        }
        safe_fs::rename_no_replace_in(&parent, backup_leaf, &target_leaf)
            .map_err(Refusal::Restore)?;
        held::open(&self.directory, &self.path, relative, usize::MAX)
    }

    pub(crate) fn remove_recovery_temp(
        &self,
        relative: &Utf8Path,
        prepared_leaf: &str,
        expected_identity: &FileIdentity,
        expected_digest: [u8; 32],
    ) -> Result<(), Refusal> {
        validate_relative(relative).map_err(Refusal::from)?;
        let (parent, target_leaf) =
            platform::open_parent(&self.directory, relative).map_err(Refusal::from)?;
        if !valid_artifact_leaf(&target_leaf, prepared_leaf, ".tmp") {
            return Err(Refusal::Changed);
        }
        let file = match platform::regular(&parent, prepared_leaf).map_err(Refusal::from) {
            Err(Refusal::Missing) => return Ok(()),
            Err(error) => return Err(error),
            Ok(file) => file,
        };
        let (bytes, found_identity, _) = read(file, usize::MAX)?;
        if &found_identity != expected_identity
            || <[u8; 32]>::from(Sha256::digest(bytes)) != expected_digest
        {
            return Err(Refusal::Changed);
        }
        fs_at::OpenOptions::default()
            .unlink_at(&parent, prepared_leaf)
            .map_err(Refusal::Write)
    }
}
