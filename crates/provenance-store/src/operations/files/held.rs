use super::held_io::{read, read_clone};
use super::held_metadata::{identity, FileMetadata};
use super::{platform, safe_fs, RepositoryFileRefusal as Refusal, Utf8Path, Utf8PathBuf};
use sha2::{Digest as _, Sha256};
use std::fs::File;
use std::io::Write as _;
use std::path::PathBuf;

pub use super::held_metadata::FileIdentity;
pub use super::held_recovery::RepositoryFileBackup;

pub struct HeldRepositoryFile {
    relative: Utf8PathBuf,
    parent: File,
    parent_path: PathBuf,
    leaf: String,
    bytes: Vec<u8>,
    digest: [u8; 32],
    identity: FileIdentity,
    metadata: FileMetadata,
}

pub struct PreparedRepositoryFile {
    parent: File,
    parent_identity: FileIdentity,
    leaf: String,
    file: File,
    identity: FileIdentity,
    digest: [u8; 32],
    target_leaf: String,
    target_identity: FileIdentity,
    target_digest: [u8; 32],
}

impl HeldRepositoryFile {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    pub const fn identity(&self) -> &FileIdentity {
        &self.identity
    }

    pub fn relative(&self) -> &Utf8Path {
        &self.relative
    }

    pub(super) const fn parent(&self) -> &File {
        &self.parent
    }

    pub(super) fn leaf(&self) -> &str {
        &self.leaf
    }

    pub fn recovery_backup(&self) -> RepositoryFileBackup {
        let leaf = format!(
            ".{}.provenance-{}.backup",
            self.leaf,
            uuid::Uuid::new_v4().simple()
        );
        RepositoryFileBackup::new(leaf, self.identity.clone(), self.digest)
    }

    /// A new random name for a prepared file next to this file.
    pub(crate) fn new_temp_leaf(&self) -> String {
        format!(
            ".{}.provenance-{}.tmp",
            self.leaf,
            uuid::Uuid::new_v4().simple()
        )
    }

    /// Makes the prepared file `leaf` next to this file with `bytes`, and
    /// syncs it. The call fails when `leaf` exists, and removes the file that
    /// it made when a later step fails. The caller records `leaf` before the
    /// call and removes the file through that record: the returned file stays
    /// when it drops.
    pub(crate) fn create_temp_at(
        &self,
        leaf: &str,
        bytes: &[u8],
    ) -> Result<PreparedRepositoryFile, Refusal> {
        let parent_identity = identity(&self.parent)?;
        let mut file = create_new(&self.parent, leaf).map_err(write_refusal)?;
        let mut cleanup = CreatedLeaf::new(&self.parent, leaf);
        file.write_all(bytes).map_err(Refusal::Write)?;
        self.metadata.apply(&file)?;
        file.sync_all().map_err(Refusal::Write)?;
        probe_io("repository_file_clone_parent").map_err(Refusal::Write)?;
        let parent = self.parent.try_clone().map_err(Refusal::Write)?;
        let file_identity = identity(&file)?;
        cleanup.disarm();
        Ok(PreparedRepositoryFile {
            parent,
            parent_identity,
            leaf: leaf.to_owned(),
            file,
            identity: file_identity,
            digest: Sha256::digest(bytes).into(),
            target_leaf: self.leaf.clone(),
            target_identity: self.identity.clone(),
            target_digest: self.digest,
        })
    }

    pub(crate) fn recover_temp(
        &self,
        leaf: &str,
        expected_identity: &FileIdentity,
        expected_digest: [u8; 32],
    ) -> Result<Option<PreparedRepositoryFile>, Refusal> {
        if !valid_artifact_leaf(&self.leaf, leaf, ".tmp") {
            return Err(Refusal::Changed);
        }
        let mut file = match platform::regular(&self.parent, leaf).map_err(Refusal::from) {
            Ok(file) => file,
            Err(Refusal::Missing) => return Ok(None),
            Err(error) => return Err(error),
        };
        let (bytes, found_identity, _) = read_clone(&mut file, usize::MAX)?;
        if &found_identity != expected_identity
            || <[u8; 32]>::from(Sha256::digest(bytes)) != expected_digest
        {
            return Err(Refusal::Changed);
        }
        Ok(Some(PreparedRepositoryFile {
            parent: self.parent.try_clone().map_err(Refusal::Read)?,
            parent_identity: identity(&self.parent)?,
            leaf: leaf.to_owned(),
            file,
            identity: found_identity,
            digest: expected_digest,
            target_leaf: self.leaf.clone(),
            target_identity: self.identity.clone(),
            target_digest: self.digest,
        }))
    }

    /// Renames this file to `backup` and `prepared` onto this file. Each
    /// failure before the rename of `prepared` puts this file back and gives
    /// the refusal. When the rename back fails, the call gives `Restore`, and
    /// `backup` holds this file. After the rename of `prepared` the install
    /// goes only forward: the caller removes `backup` with
    /// [`Self::remove_own_backup`].
    pub(crate) fn install(
        &self,
        prepared: PreparedRepositoryFile,
        backup: &RepositoryFileBackup,
    ) -> Result<(), Refusal> {
        self.require_prepared_for_target(&prepared)?;
        if backup.identity() != &self.identity
            || backup.digest() != self.digest
            || !valid_artifact_leaf(&self.leaf, backup.leaf(), ".backup")
        {
            return Err(Refusal::Changed);
        }
        self.displace_to_backup(backup.leaf())?;
        probe_io("repository_file_after_displace").map_err(Refusal::Write)?;
        self.require_matching_backup(backup.leaf())?;
        self.install_prepared(backup.leaf(), prepared)
    }

    fn require_prepared_for_target(
        &self,
        prepared: &PreparedRepositoryFile,
    ) -> Result<(), Refusal> {
        let matches = self.anchor_matches()
            && identity(&self.parent)? == prepared.parent_identity
            && prepared.target_leaf == self.leaf
            && prepared.target_identity == self.identity
            && prepared.target_digest == self.digest;
        matches.then_some(()).ok_or(Refusal::Changed)
    }

    fn require_matching_backup(&self, backup: &str) -> Result<(), Refusal> {
        match self.matches_backup(backup) {
            Ok(true) => Ok(()),
            Ok(false) => self.restore_then_refuse(backup, Refusal::Changed),
            Err(error) => self.restore_then_refuse(backup, error),
        }
    }

    fn install_prepared(
        &self,
        backup: &str,
        mut prepared: PreparedRepositoryFile,
    ) -> Result<(), Refusal> {
        let install = (|| {
            probe_io("repository_file_after_backup_check").map_err(Refusal::Write)?;
            if !prepared.matches_entry()? {
                return Err(Refusal::Changed);
            }
            safe_fs::rename_no_replace_in(&prepared.parent, &prepared.leaf, &self.leaf)
                .map_err(write_refusal)
        })();
        install.or_else(|error| self.restore_then_refuse(backup, error))
    }

    fn restore_then_refuse(&self, backup: &str, refusal: Refusal) -> Result<(), Refusal> {
        restore(&self.parent, backup, &self.leaf)?;
        Err(refusal)
    }

    fn displace_to_backup(&self, backup: &str) -> Result<(), Refusal> {
        match safe_fs::rename_no_replace_in(&self.parent, &self.leaf, backup) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(Refusal::Changed),
            Err(error) => Err(write_refusal(error)),
        }
    }

    fn matches_backup(&self, backup: &str) -> Result<bool, Refusal> {
        let file = match platform::regular(&self.parent, backup).map_err(Refusal::from) {
            Ok(file) => file,
            Err(Refusal::Denied | Refusal::Missing) => return Ok(false),
            Err(error) => return Err(error),
        };
        let (bytes, current_identity, _) = match read(file, self.bytes.len()) {
            Ok(read) => read,
            Err(Refusal::TooLarge { .. } | Refusal::InvalidUtf8) => return Ok(false),
            Err(error) => return Err(error),
        };
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        Ok(current_identity == self.identity && digest == self.digest)
    }

    fn anchor_matches(&self) -> bool {
        safe_fs::Directory::open(&self.parent_path, "repository file parent")
            .and_then(|directory| {
                identity(directory.as_file())
                    .map_err(|error| std::io::Error::other(error.to_string()))
            })
            .is_ok_and(|current| identity(&self.parent).is_ok_and(|held| current == held))
    }
}

impl PreparedRepositoryFile {
    pub(crate) const fn recovery_identity(&self) -> &FileIdentity {
        &self.identity
    }

    fn matches_entry(&mut self) -> Result<bool, Refusal> {
        let entry = match platform::regular(&self.parent, &self.leaf).map_err(Refusal::from) {
            Ok(file) => file,
            Err(Refusal::Denied | Refusal::Missing) => return Ok(false),
            Err(error) => return Err(error),
        };
        let (entry_bytes, entry_identity, _) = match read(entry, usize::MAX) {
            Ok(read) => read,
            Err(Refusal::TooLarge { .. } | Refusal::InvalidUtf8) => return Ok(false),
            Err(error) => return Err(error),
        };
        let (held_bytes, held_identity, _) = read_clone(&mut self.file, usize::MAX)?;
        Ok(entry_identity == self.identity
            && held_identity == self.identity
            && Sha256::digest(entry_bytes).as_slice() == self.digest
            && Sha256::digest(held_bytes).as_slice() == self.digest)
    }
}

pub(super) fn open(
    root: &File,
    root_path: &Utf8Path,
    relative: &Utf8Path,
    limit: usize,
) -> Result<HeldRepositoryFile, Refusal> {
    let (parent, leaf) = platform::open_parent(root, relative).map_err(Refusal::from)?;
    let file = platform::regular(&parent, &leaf).map_err(Refusal::from)?;
    let (bytes, identity, metadata) = read(file, limit)?;
    Ok(HeldRepositoryFile {
        relative: relative.to_owned(),
        parent,
        parent_path: {
            let mut path = root_path.to_path_buf();
            path.extend(relative.parent().into_iter().flat_map(Utf8Path::components));
            path.into_std_path_buf()
        },
        leaf,
        digest: Sha256::digest(&bytes).into(),
        bytes,
        identity,
        metadata,
    })
}

fn create_new(parent: &File, leaf: &str) -> std::io::Result<File> {
    let mut options = fs_at::OpenOptions::default();
    #[cfg(windows)]
    {
        use fs_at::os::windows::OpenOptionsExt as _;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_GENERIC_READ, FILE_GENERIC_WRITE, WRITE_DAC,
        };
        options.desired_access(FILE_GENERIC_READ | FILE_GENERIC_WRITE | WRITE_DAC);
    }
    options
        .read(true)
        .write(fs_at::OpenOptionsWriteMode::Write)
        .create_new(true)
        .follow(false);
    options.open_at(parent, leaf)
}

fn unlink(parent: &File, leaf: &str) -> std::io::Result<()> {
    fs_at::OpenOptions::default().unlink_at(parent, leaf)
}

fn restore(parent: &File, backup: &str, leaf: &str) -> Result<(), Refusal> {
    probe_io("repository_file_restore").map_err(Refusal::Restore)?;
    safe_fs::rename_no_replace_in(parent, backup, leaf).map_err(Refusal::Restore)
}

pub(super) fn unlink_temp(parent: &File, leaf: &str) -> std::io::Result<()> {
    probe_io("repository_file_unlink_temp")?;
    unlink(parent, leaf)
}

pub(super) fn probe_io(label: &str) -> std::io::Result<()> {
    crate::test_probes::at(label).map_err(std::io::Error::other)
}

struct CreatedLeaf<'a> {
    parent: &'a File,
    leaf: &'a str,
    armed: bool,
}

impl<'a> CreatedLeaf<'a> {
    const fn new(parent: &'a File, leaf: &'a str) -> Self {
        Self {
            parent,
            leaf,
            armed: true,
        }
    }

    const fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for CreatedLeaf<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = unlink_temp(self.parent, self.leaf);
        }
    }
}

fn write_refusal(error: std::io::Error) -> Refusal {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied
        | std::io::ErrorKind::NotADirectory
        | std::io::ErrorKind::InvalidInput => Refusal::Denied,
        _ => Refusal::Write(error),
    }
}

pub(super) fn valid_artifact_leaf(target: &str, leaf: &str, suffix: &str) -> bool {
    if matches!(leaf, "" | "." | "..") || leaf.contains(['/', '\\', ':', '\0']) {
        return false;
    }
    let prefix = format!(".{target}.provenance-");
    leaf.strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix(suffix))
        .is_some_and(|token| {
            token.len() == 32 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}
