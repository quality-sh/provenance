//! Held-directory traversal for Windows. The traversal also runs in Unix tests.
use super::{FileAccessRefusal as Refusal, Utf8Path};
use provenance_scanner::{FileScan, Language};
use std::{
    fs::{File, Metadata},
    io::{self, Read},
};

#[cfg(windows)]
pub(super) fn open_root(path: &Utf8Path) -> Result<File, Refusal> {
    use std::os::windows::fs::OpenOptionsExt as _;
    use std::path::{Component, Prefix};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    };
    let mut components = path.as_std_path().components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err(Refusal::Denied);
    };
    if !matches!(
        prefix.kind(),
        Prefix::Disk(_) | Prefix::VerbatimDisk(_) | Prefix::UNC(..) | Prefix::VerbatimUNC(..)
    ) || components.next() != Some(Component::RootDir)
    {
        return Err(Refusal::Denied);
    }
    // Only the trusted drive/share anchor uses a global path. Every descendant
    // is opened by one name relative to its held parent.
    let mut anchor = std::path::PathBuf::from(prefix.as_os_str());
    anchor.push(std::path::MAIN_SEPARATOR_STR);
    let mut held = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(anchor)
        .map_err(error)?;
    require_directory(&held)?;
    for component in components {
        let Component::Normal(name) = component else {
            return Err(Refusal::Denied);
        };
        held = child(&held, name.to_str().ok_or(Refusal::Denied)?, Access::Read)?;
        require_directory(&held)?;
    }
    Ok(held)
}

#[cfg(unix)]
pub(super) fn open_root(path: &Utf8Path) -> Result<File, Refusal> {
    super::unix::open_root(path)
}

pub(super) fn open_file(root: &File, relative: &Utf8Path) -> Result<File, Refusal> {
    super::validate_relative(relative)?;
    let mut held = root.try_clone().map_err(error)?;
    let mut parts = relative.as_str().split('/').peekable();
    while let Some(name) = parts.next() {
        held = child(&held, name, Access::Read)?;
        if parts.peek().is_some() {
            require_directory(&held)?;
        }
    }
    if !metadata(&held)?.is_file() {
        return Err(Refusal::Denied);
    }
    Ok(held)
}

pub(super) fn scan_tree(
    root: &File,
    path: &Utf8Path,
    limit: usize,
) -> Result<(Vec<FileScan>, bool), Refusal> {
    let mut scans = Vec::new();
    let mut frames = vec![Frame::new(
        root.try_clone().map_err(error)?,
        path.to_owned(),
    )?];
    let mut cut = false;
    while let Some(frame) = frames.last_mut() {
        let Some(name) = frame.names.next() else {
            frames.pop();
            continue;
        };
        if matches!(
            name.as_str(),
            "." | ".." | ".git" | "node_modules" | "target"
        ) {
            continue;
        }
        let inspected = open_child(&frame.directory, &name, Access::Attributes)?;
        let kind = inspected.metadata().map_err(error)?;
        let path = frame.path.join(&name);
        if is_reparse(&kind) {
            if path
                .extension()
                .and_then(Language::from_extension)
                .is_none()
            {
                continue;
            }
            return Err(Refusal::Denied);
        }
        if kind.is_dir() {
            let directory = child(&frame.directory, &name, Access::Read)?;
            frames.push(Frame::new(directory, path)?);
        } else if let Some(language) = path.extension().and_then(Language::from_extension) {
            if !kind.is_file() {
                return Err(Refusal::Denied);
            }
            if scans.len() == limit {
                cut = true;
                break;
            }
            let mut file = child(&frame.directory, &name, Access::Read)?;
            if !metadata(&file)?.is_file() {
                return Err(Refusal::Denied);
            }
            let mut contents = String::new();
            file.read_to_string(&mut contents).map_err(error)?;
            scans.push(provenance_scanner::scan_file(&path, language, &contents));
        }
    }
    scans.sort_by(|left, right| left.file_path.cmp(&right.file_path));
    Ok((scans, cut))
}

struct Frame {
    directory: File,
    path: camino::Utf8PathBuf,
    names: std::vec::IntoIter<String>,
}
impl Frame {
    fn new(mut directory: File, path: camino::Utf8PathBuf) -> Result<Self, Refusal> {
        require_directory(&directory)?;
        let names = fs_at::read_dir(&mut directory)
            .map_err(error)?
            .map(|entry| {
                entry
                    .map_err(error)
                    .map(|entry| entry.name().to_str().map(str::to_owned))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut names = names.into_iter().flatten().collect::<Vec<_>>();
        names.sort();
        Ok(Self {
            directory,
            path,
            names: names.into_iter(),
        })
    }
}

#[derive(Clone, Copy)]
enum Access {
    Attributes,
    Read,
}

fn child(parent: &File, name: &str, access: Access) -> Result<File, Refusal> {
    let file = open_child(parent, name, access)?;
    metadata(&file)?;
    Ok(file)
}

fn open_child(parent: &File, name: &str, access: Access) -> Result<File, Refusal> {
    let mut options = fs_at::OpenOptions::default();
    #[cfg(windows)]
    let file = {
        use fs_at::os::windows::OpenOptionsExt as _;
        use windows_sys::Win32::Storage::FileSystem::{FILE_GENERIC_READ, FILE_READ_ATTRIBUTES};
        // open_path_at returns the reparse object as an owned handle. Inspect
        // it ourselves so every refusal closes that handle, for every tag.
        const OBJ_CASE_INSENSITIVE: u32 = 0x40;
        options.object_attributes(OBJ_CASE_INSENSITIVE);
        options.desired_access(match access {
            Access::Attributes => FILE_READ_ATTRIBUTES,
            Access::Read => FILE_GENERIC_READ,
        });
        options.open_path_at(parent, name).map_err(error)?
    };
    #[cfg(unix)]
    let file = match access {
        Access::Attributes => options
            .follow(false)
            .open_path_at(parent, name)
            .map_err(error)?,
        Access::Read => options
            .read(true)
            .follow(false)
            .open_at(parent, name)
            .map_err(error)?,
    };
    Ok(file)
}

fn metadata(file: &File) -> Result<Metadata, Refusal> {
    let metadata = file.metadata().map_err(error)?;
    if is_reparse(&metadata) {
        return Err(Refusal::Denied);
    }
    Ok(metadata)
}

fn is_reparse(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(unix)]
    {
        metadata.file_type().is_symlink()
    }
}

fn require_directory(file: &File) -> Result<(), Refusal> {
    if metadata(file)?.is_dir() {
        Ok(())
    } else {
        Err(Refusal::Denied)
    }
}

fn error(error: io::Error) -> Refusal {
    #[cfg(unix)]
    if error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error()) {
        return Refusal::Denied;
    }
    match error.kind() {
        io::ErrorKind::NotFound => Refusal::Missing,
        io::ErrorKind::PermissionDenied
        | io::ErrorKind::NotADirectory
        | io::ErrorKind::InvalidInput => Refusal::Denied,
        _ => Refusal::Read(error),
    }
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
