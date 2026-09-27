use super::{platform, safe_fs, RepositoryFileRefusal as Refusal, Utf8Path, Utf8PathBuf};
use sha2::{Digest as _, Sha256};
use std::fs::{File, Permissions};
use std::io::{Read as _, Write as _};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileIdentity {
    volume: u64,
    file: u64,
}

pub struct HeldRepositoryFile {
    relative: Utf8PathBuf,
    parent: File,
    parent_path: PathBuf,
    leaf: String,
    bytes: Vec<u8>,
    digest: [u8; 32],
    identity: FileIdentity,
    permissions: Permissions,
}

pub struct PreparedRepositoryFile {
    parent: File,
    parent_identity: FileIdentity,
    leaf: String,
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
                    let prepared = (|| {
                        file.write_all(bytes).map_err(Refusal::Write)?;
                        file.set_permissions(self.permissions.clone())
                            .map_err(Refusal::Write)?;
                        file.sync_all().map_err(Refusal::Write)
                    })();
                    if let Err(error) = prepared {
                        let _ = unlink(&self.parent, &leaf);
                        return Err(error);
                    }
                    return Ok(PreparedRepositoryFile {
                        parent: self.parent.try_clone().map_err(Refusal::Write)?,
                        parent_identity,
                        leaf,
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

    pub fn compare_and_swap(&self, mut prepared: PreparedRepositoryFile) -> Result<(), Refusal> {
        if identity(&self.parent)? != prepared.parent_identity {
            return Err(Refusal::Denied);
        }
        let backup = self.displace_to_backup()?;
        match self.matches_backup(&backup) {
            Ok(true) => {}
            Ok(false) => {
                restore(&self.parent, &self.parent_path, &backup, &self.leaf)?;
                return Err(Refusal::Changed);
            }
            Err(error) => {
                restore(&self.parent, &self.parent_path, &backup, &self.leaf)?;
                return Err(error);
            }
        }
        if let Err(error) = safe_fs::rename_no_replace_in(
            &prepared.parent,
            &self.parent_path,
            &prepared.leaf,
            &self.leaf,
        ) {
            restore(&self.parent, &self.parent_path, &backup, &self.leaf)?;
            return Err(write_refusal(error));
        }
        prepared.remove_on_drop = false;
        unlink(&self.parent, &backup).map_err(Refusal::Write)
    }

    fn displace_to_backup(&self) -> Result<String, Refusal> {
        for _ in 0..100 {
            let backup = format!(
                ".{}.provenance-{}.backup",
                self.leaf,
                uuid::Uuid::new_v4().simple()
            );
            match safe_fs::rename_no_replace_in(
                &self.parent,
                &self.parent_path,
                &self.leaf,
                &backup,
            ) {
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
}

impl Drop for PreparedRepositoryFile {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _ = unlink(&self.parent, &self.leaf);
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
    let (bytes, identity, permissions) = read(file, limit)?;
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
        permissions,
    })
}

fn read(mut file: File, limit: usize) -> Result<(Vec<u8>, FileIdentity, Permissions), Refusal> {
    let metadata = file.metadata().map_err(Refusal::Read)?;
    if metadata.len() > u64::try_from(limit).unwrap_or(u64::MAX) {
        return Err(Refusal::TooLarge { limit });
    }
    let identity = identity(&file)?;
    let permissions = metadata.permissions();
    let read_limit = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
    std::io::Read::by_ref(&mut file)
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(Refusal::Read)?;
    if bytes.len() > limit {
        return Err(Refusal::TooLarge { limit });
    }
    std::str::from_utf8(&bytes).map_err(|_| Refusal::InvalidUtf8)?;
    Ok((bytes, identity, permissions))
}

fn create_new(parent: &File, leaf: &str) -> std::io::Result<File> {
    let mut options = fs_at::OpenOptions::default();
    options
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
    parent_path: &std::path::Path,
    backup: &str,
    leaf: &str,
) -> Result<(), Refusal> {
    safe_fs::rename_no_replace_in(parent, parent_path, backup, leaf).map_err(Refusal::Restore)
}

fn write_refusal(error: std::io::Error) -> Refusal {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied
        | std::io::ErrorKind::NotADirectory
        | std::io::ErrorKind::InvalidInput => Refusal::Denied,
        _ => Refusal::Write(error),
    }
}

#[cfg(unix)]
fn identity(file: &File) -> Result<FileIdentity, Refusal> {
    use std::os::unix::fs::MetadataExt as _;
    let metadata = file.metadata().map_err(Refusal::Read)?;
    Ok(FileIdentity {
        volume: metadata.dev(),
        file: metadata.ino(),
    })
}

#[cfg(windows)]
fn identity(file: &File) -> Result<FileIdentity, Refusal> {
    use std::mem::MaybeUninit;
    use std::os::windows::io::AsRawHandle as _;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    if unsafe {
        GetFileInformationByHandle(file.as_raw_handle() as HANDLE, information.as_mut_ptr())
    } == 0
    {
        return Err(Refusal::Read(std::io::Error::last_os_error()));
    }
    let information = unsafe { information.assume_init() };
    Ok(FileIdentity {
        volume: u64::from(information.dwVolumeSerialNumber),
        file: (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow),
    })
}
