use super::held_metadata::{identity, FileMetadata};
use super::{platform, safe_fs, RepositoryFileRefusal as Refusal, Utf8Path, Utf8PathBuf};
use sha2::{Digest as _, Sha256};
use std::fs::File;
use std::io::{Read as _, Seek as _, Write as _};
use std::path::PathBuf;

pub use super::held_metadata::FileIdentity;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackupRetentionReason {
    OriginalChanged,
    VerificationFailed,
    RemovalFailed,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RepositoryFileInstall {
    Installed,
    InstalledBackupKept {
        backup: String,
        reason: BackupRetentionReason,
    },
}

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
    remove_on_drop: bool,
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

    pub fn create_temp(&self, bytes: &[u8]) -> Result<PreparedRepositoryFile, Refusal> {
        let parent_identity = identity(&self.parent)?;
        for _ in 0..100 {
            let leaf = format!(
                ".{}.provenance-{}.tmp",
                self.leaf,
                uuid::Uuid::new_v4().simple()
            );
            match create_new(&self.parent, &leaf) {
                Ok(mut file) => {
                    let mut cleanup = CreatedLeaf::new(&self.parent, &leaf);
                    let prepared: Result<(File, FileIdentity), Refusal> = (|| {
                        file.write_all(bytes).map_err(Refusal::Write)?;
                        self.metadata.apply(&file)?;
                        file.sync_all().map_err(Refusal::Write)?;
                        probe_io("repository_file_clone_parent").map_err(Refusal::Write)?;
                        let parent = self.parent.try_clone().map_err(Refusal::Write)?;
                        let file_identity = identity(&file)?;
                        Ok((parent, file_identity))
                    })();
                    let (parent, file_identity) = prepared?;
                    cleanup.disarm();
                    drop(cleanup);
                    return Ok(PreparedRepositoryFile {
                        parent,
                        parent_identity,
                        leaf,
                        file,
                        identity: file_identity,
                        digest: Sha256::digest(bytes).into(),
                        target_leaf: self.leaf.clone(),
                        target_identity: self.identity.clone(),
                        target_digest: self.digest,
                        remove_on_drop: true,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(write_refusal(error)),
            }
        }
        Err(Refusal::Write(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not allocate a repository temporary file",
        )))
    }

    pub fn compare_and_swap(
        &self,
        mut prepared: PreparedRepositoryFile,
    ) -> Result<RepositoryFileInstall, Refusal> {
        if !self.anchor_matches()
            || identity(&self.parent)? != prepared.parent_identity
            || prepared.target_leaf != self.leaf
            || prepared.target_identity != self.identity
            || prepared.target_digest != self.digest
        {
            return Err(Refusal::Changed);
        }
        let backup = self.displace_to_backup()?;
        match self.matches_backup(&backup) {
            Ok(true) => {}
            Ok(false) => {
                restore(&self.parent, &backup, &self.leaf)?;
                return Err(Refusal::Changed);
            }
            Err(error) => {
                restore(&self.parent, &backup, &self.leaf)?;
                return Err(error);
            }
        }
        if let Err(error) = probe_io("repository_file_after_backup_check") {
            restore(&self.parent, &backup, &self.leaf)?;
            return Err(Refusal::Write(error));
        }
        if !prepared.matches_entry()? {
            restore(&self.parent, &backup, &self.leaf)?;
            return Err(Refusal::Changed);
        }
        if let Err(error) =
            safe_fs::rename_no_replace_in(&prepared.parent, &prepared.leaf, &self.leaf)
        {
            restore(&self.parent, &backup, &self.leaf)?;
            return Err(write_refusal(error));
        }
        prepared.remove_on_drop = false;
        match self.matches_backup(&backup) {
            Ok(true) => {}
            Ok(false) => {
                return Ok(RepositoryFileInstall::InstalledBackupKept {
                    backup,
                    reason: BackupRetentionReason::OriginalChanged,
                });
            }
            Err(_) => {
                return Ok(RepositoryFileInstall::InstalledBackupKept {
                    backup,
                    reason: BackupRetentionReason::VerificationFailed,
                });
            }
        }
        if probe_io("repository_file_unlink_backup").is_err()
            || unlink(&self.parent, &backup).is_err()
        {
            return Ok(RepositoryFileInstall::InstalledBackupKept {
                backup,
                reason: BackupRetentionReason::RemovalFailed,
            });
        }
        Ok(RepositoryFileInstall::Installed)
    }

    fn displace_to_backup(&self) -> Result<String, Refusal> {
        for _ in 0..100 {
            let backup = format!(
                ".{}.provenance-{}.backup",
                self.leaf,
                uuid::Uuid::new_v4().simple()
            );
            match safe_fs::rename_no_replace_in(&self.parent, &self.leaf, &backup) {
                Ok(()) => return Ok(backup),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Err(Refusal::Changed);
                }
                Err(error) => return Err(write_refusal(error)),
            }
        }
        Err(Refusal::Write(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not allocate a repository backup file",
        )))
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
                identity(directory.as_file()).map_err(|error| std::io::Error::other(error.to_string()))
            })
            .is_ok_and(|current| identity(&self.parent).is_ok_and(|held| current == held))
    }
}

impl PreparedRepositoryFile {
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

impl Drop for PreparedRepositoryFile {
    fn drop(&mut self) {
        if self.remove_on_drop && entry_has_identity(&self.parent, &self.leaf, &self.identity) {
            let _ = unlink_temp(&self.parent, &self.leaf);
        }
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
        parent_path: root_path
            .join(relative.parent().unwrap_or_else(|| Utf8Path::new("")))
            .into_std_path_buf(),
        leaf,
        digest: Sha256::digest(&bytes).into(),
        bytes,
        identity,
        metadata,
    })
}

fn read(mut file: File, limit: usize) -> Result<(Vec<u8>, FileIdentity, FileMetadata), Refusal> {
    read_inner(&mut file, limit)
}

fn read_clone(
    file: &mut File,
    limit: usize,
) -> Result<(Vec<u8>, FileIdentity, FileMetadata), Refusal> {
    read_inner(file, limit)
}

fn read_inner(file: &mut File, limit: usize) -> Result<(Vec<u8>, FileIdentity, FileMetadata), Refusal> {
    let metadata = file.metadata().map_err(Refusal::Read)?;
    if metadata.len() > u64::try_from(limit).unwrap_or(u64::MAX) {
        return Err(Refusal::TooLarge { limit });
    }
    let identity = identity(&file)?;
    let file_metadata = FileMetadata::read(file)?;
    probe_io("repository_file_after_metadata").map_err(Refusal::Read)?;
    file.rewind().map_err(Refusal::Read)?;
    let read_limit = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
    std::io::Read::by_ref(&mut *file)
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(Refusal::Read)?;
    if bytes.len() > limit {
        return Err(Refusal::TooLarge { limit });
    }
    std::str::from_utf8(&bytes).map_err(|_| Refusal::InvalidUtf8)?;
    Ok((bytes, identity, file_metadata))
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

fn restore(
    parent: &File,
    backup: &str,
    leaf: &str,
) -> Result<(), Refusal> {
    safe_fs::rename_no_replace_in(parent, backup, leaf).map_err(Refusal::Restore)
}

fn entry_has_identity(parent: &File, leaf: &str, expected: &FileIdentity) -> bool {
    platform::regular(parent, leaf)
        .ok()
        .and_then(|file| identity(&file).ok())
        .is_some_and(|current| &current == expected)
}

fn unlink_temp(parent: &File, leaf: &str) -> std::io::Result<()> {
    probe_io("repository_file_unlink_temp")?;
    unlink(parent, leaf)
}

fn probe_io(label: &str) -> std::io::Result<()> {
    crate::test_probes::at(label).map_err(std::io::Error::other)
}

struct CreatedLeaf<'a> {
    parent: &'a File,
    leaf: &'a str,
    armed: bool,
}

impl<'a> CreatedLeaf<'a> {
    const fn new(parent: &'a File, leaf: &'a str) -> Self {
        Self { parent, leaf, armed: true }
    }

    fn disarm(&mut self) {
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
