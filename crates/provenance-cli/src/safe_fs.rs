//! Filesystem operations that refuse links and replacement races.

use std::fs::File;
use std::path::{Path, PathBuf};

/// A directory handle and the path that anchored it.
pub(super) struct Directory {
    file: File,
    path: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ChildKind {
    Directory,
    File,
    Symlink,
    Other,
}

impl Directory {
    pub(super) fn open(path: &Path) -> std::io::Result<Self> {
        open_directory_no_follow(path).map(|file| Self {
            file,
            path: path.to_path_buf(),
        })
    }

    pub(super) const fn from_file(file: File, path: PathBuf) -> Self {
        Self { file, path }
    }

    pub(super) const fn as_file(&self) -> &File {
        &self.file
    }

    pub(super) fn try_clone(&self) -> std::io::Result<Self> {
        Ok(Self {
            file: self.file.try_clone()?,
            path: self.path.clone(),
        })
    }

    pub(super) fn into_file(self) -> File {
        self.file
    }

    pub(super) fn open_child(&self, leaf: &str) -> std::io::Result<Self> {
        open_child_directory_no_follow(&self.file, leaf).map(|file| Self {
            file,
            path: self.path.join(leaf),
        })
    }

    pub(super) fn create_child(&self, leaf: &str) -> std::io::Result<Self> {
        fs_at::OpenOptions::default()
            .mkdir_at(&self.file, leaf)
            .map(|file| Self {
                file,
                path: self.path.join(leaf),
            })
    }

    pub(super) fn child_kind(&self, leaf: &str) -> std::io::Result<Option<ChildKind>> {
        child_kind(&self.file, leaf)
    }

    pub(super) fn rename_no_replace(&self, from: &str, to: &str) -> std::io::Result<()> {
        rename_no_replace_at(&self.file, &self.path, from, to)
    }
}

#[cfg(unix)]
fn child_kind(parent: &File, leaf: &str) -> std::io::Result<Option<ChildKind>> {
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
fn child_kind(parent: &File, leaf: &str) -> std::io::Result<Option<ChildKind>> {
    #[cfg(windows)]
    use std::os::windows::fs::MetadataExt;
    let mut options = fs_at::OpenOptions::default();
    options.follow(false);
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

#[cfg(unix)]
fn open_child_directory_no_follow(parent: &File, leaf: &str) -> std::io::Result<File> {
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
fn open_child_directory_no_follow(parent: &File, leaf: &str) -> std::io::Result<File> {
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
fn open_child_directory_no_follow(parent: &File, leaf: &str) -> std::io::Result<File> {
    let mut options = fs_at::OpenOptions::default();
    options.follow(false);
    options.open_dir_at(parent, leaf)
}

#[cfg(unix)]
fn open_directory_no_follow(path: &Path) -> std::io::Result<File> {
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
fn open_directory_no_follow(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

    let directory = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0x1 | 0x2 | 0x4)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    if directory.metadata()?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "directory is a reparse point",
        ));
    }
    Ok(directory)
}

#[cfg(not(any(unix, windows)))]
fn open_directory_no_follow(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
pub(super) fn rename_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    rustix_rename_no_replace(rustix::fs::CWD, from, rustix::fs::CWD, to)
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
fn rustix_rename_no_replace<Fd1: rustix::fd::AsFd, Fd2: rustix::fd::AsFd>(
    from_dir: Fd1,
    from: impl rustix::path::Arg,
    to_dir: Fd2,
    to: impl rustix::path::Arg,
) -> std::io::Result<()> {
    rustix::fs::renameat_with(
        from_dir,
        from,
        to_dir,
        to,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)
}

#[cfg(target_os = "macos")]
pub(super) fn rename_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    rename_no_replace_at_raw(-2, from.as_os_str(), -2, to.as_os_str())
}

#[cfg(target_os = "macos")]
fn rename_no_replace_at_raw(
    from_dir: std::os::raw::c_int,
    from: &std::ffi::OsStr,
    to_dir: std::os::raw::c_int,
    to: &std::ffi::OsStr,
) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int, c_uint};
    use std::os::unix::ffi::OsStrExt;

    extern "C" {
        fn renameatx_np(
            from_dir: c_int,
            from: *const c_char,
            to_dir: c_int,
            to: *const c_char,
            flags: c_uint,
        ) -> c_int;
    }

    let from = CString::new(from.as_bytes()).map_err(|_| std::io::ErrorKind::InvalidInput)?;
    let to = CString::new(to.as_bytes()).map_err(|_| std::io::ErrorKind::InvalidInput)?;
    if unsafe { renameatx_np(from_dir, from.as_ptr(), to_dir, to.as_ptr(), 0x0000_0004) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(windows)]
pub(super) fn rename_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, ERROR_FILE_EXISTS};
    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

    let from: Vec<u16> = from.as_os_str().encode_wide().chain(once(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(once(0)).collect();
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0) } != 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(code) if code == ERROR_FILE_EXISTS as i32 || code == ERROR_ALREADY_EXISTS as i32 => {
            Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                error,
            ))
        }
        _ => Err(error),
    }
}

#[cfg(windows)]
fn rename_no_replace_at(
    parent: &File,
    parent_path: &Path,
    from: &str,
    to: &str,
) -> std::io::Result<()> {
    use fs_at::os::windows::OpenOptionsExt;
    use std::ffi::OsStr;
    use std::mem::size_of;
    use std::os::windows::{ffi::OsStrExt, io::AsRawHandle};
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Storage::FileSystem::{
        FileRenameInfoEx, SetFileInformationByHandle, FILE_RENAME_INFO, FILE_RENAME_INFO_0,
    };

    const DELETE_ACCESS: u32 = 0x0001_0000;
    let mut options = fs_at::OpenOptions::default();
    options.desired_access(DELETE_ACCESS).follow(false);
    let source = options.open_path_at(parent, from)?;
    let destination = parent_path.join(to);
    let name: Vec<u16> = OsStr::new(&destination).encode_wide().collect();
    let name_bytes = name
        .len()
        .checked_mul(size_of::<u16>())
        .ok_or(std::io::ErrorKind::InvalidInput)?;
    let buffer_size = size_of::<FILE_RENAME_INFO>()
        .checked_add(name_bytes)
        .ok_or(std::io::ErrorKind::InvalidInput)?;
    let mut storage = vec![0_usize; buffer_size.div_ceil(size_of::<usize>())];
    let info = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    unsafe {
        info.write(FILE_RENAME_INFO {
            Anonymous: FILE_RENAME_INFO_0 { Flags: 0 },
            RootDirectory: 0 as HANDLE,
            FileNameLength: u32::try_from(name_bytes)
                .map_err(|_| std::io::ErrorKind::InvalidInput)?,
            FileName: [0],
        });
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            std::ptr::addr_of_mut!((*info).FileName).cast::<u16>(),
            name.len(),
        );
        if SetFileInformationByHandle(
            source.as_raw_handle() as HANDLE,
            FileRenameInfoEx,
            info.cast(),
            u32::try_from(buffer_size).map_err(|_| std::io::ErrorKind::InvalidInput)?,
        ) == 0
        {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
fn rename_no_replace_at(
    parent: &File,
    _parent_path: &Path,
    from: &str,
    to: &str,
) -> std::io::Result<()> {
    rustix_rename_no_replace(parent, from, parent, to)
}

#[cfg(target_os = "macos")]
fn rename_no_replace_at(
    parent: &File,
    _parent_path: &Path,
    from: &str,
    to: &str,
) -> std::io::Result<()> {
    use std::os::fd::AsRawFd;
    rename_no_replace_at_raw(
        parent.as_raw_fd(),
        from.as_ref(),
        parent.as_raw_fd(),
        to.as_ref(),
    )
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "macos",
    windows
)))]
pub(super) fn rename_no_replace(_from: &Path, _to: &Path) -> std::io::Result<()> {
    unsupported_rename()
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "macos",
    windows
)))]
fn rename_no_replace_at(
    _parent: &File,
    _parent_path: &Path,
    _from: &str,
    _to: &str,
) -> std::io::Result<()> {
    unsupported_rename()
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "macos",
    windows
)))]
fn unsupported_rename() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic no-replace rename is unavailable on this platform",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_interface_classifies_children() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("directory")).unwrap();
        std::fs::write(temp.path().join("file"), "content").unwrap();
        let directory = Directory::open(temp.path()).unwrap();

        assert_eq!(directory.child_kind("missing").unwrap(), None);
        assert_eq!(
            directory.child_kind("directory").unwrap(),
            Some(ChildKind::Directory)
        );
        assert_eq!(directory.child_kind("file").unwrap(), Some(ChildKind::File));
    }

    #[test]
    fn directory_rename_does_not_replace_an_existing_child() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("source"), "source").unwrap();
        std::fs::write(temp.path().join("destination"), "destination").unwrap();
        let directory = Directory::open(temp.path()).unwrap();

        let error = directory
            .rename_no_replace("source", "destination")
            .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read_to_string(temp.path().join("destination")).unwrap(),
            "destination"
        );
        assert_eq!(
            std::fs::read_to_string(temp.path().join("source")).unwrap(),
            "source"
        );
    }
}
