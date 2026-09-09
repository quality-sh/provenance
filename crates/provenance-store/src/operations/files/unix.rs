//! No-follow descriptor traversal. Root and opened descendants retain their identity.
use super::{FileAccessRefusal as Refusal, Utf8Path, Utf8PathBuf};
use provenance_scanner::{FileScan, Language};
use rustix::fs::{self, AtFlags, FileType, Mode, OFlags};
use std::{fs::File, io::Read};

fn error(error: rustix::io::Errno) -> Refusal {
    match error {
        rustix::io::Errno::NOENT => Refusal::Missing,
        rustix::io::Errno::LOOP
        | rustix::io::Errno::NOTDIR
        | rustix::io::Errno::ACCESS
        | rustix::io::Errno::PERM => Refusal::Denied,
        other => Refusal::Read(other.into()),
    }
}
fn directory(parent: &File, name: &str) -> Result<File, Refusal> {
    fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(error)
}
pub(super) fn open_root(path: &Utf8Path) -> Result<File, Refusal> {
    if !path.is_absolute() {
        return Err(Refusal::Denied);
    }
    let mut held = File::from(
        fs::open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(error)?,
    );
    for component in path.components() {
        match component {
            camino::Utf8Component::RootDir => {}
            camino::Utf8Component::Normal(name) => held = directory(&held, name)?,
            _ => return Err(Refusal::Denied),
        }
    }
    Ok(held)
}
fn regular(parent: &File, name: &str) -> Result<File, Refusal> {
    let stat = fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW).map_err(error)?;
    if !FileType::from_raw_mode(stat.st_mode).is_file() {
        return Err(Refusal::Denied);
    }
    let file = File::from(
        fs::openat(
            parent,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(error)?,
    );
    if !file.metadata().map_err(Refusal::Read)?.is_file() {
        return Err(Refusal::Denied);
    }
    Ok(file)
}
pub(super) fn open_file(root: &File, relative: &Utf8Path) -> Result<File, Refusal> {
    let mut held = root.try_clone().map_err(Refusal::Read)?;
    let components = relative.as_str().split('/').collect::<Vec<_>>();
    for name in &components[..components.len() - 1] {
        held = directory(&held, name)?;
    }
    regular(&held, components[components.len() - 1])
}
struct Frame {
    directory: File,
    path: Utf8PathBuf,
    names: std::vec::IntoIter<String>,
}
impl Frame {
    fn new(directory: File, path: Utf8PathBuf) -> Result<Self, Refusal> {
        let names = fs::Dir::read_from(&directory)
            .map_err(error)?
            .map(|entry| {
                entry
                    .map_err(error)
                    .map(|entry| entry.file_name().to_str().ok().map(str::to_owned))
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
pub(super) fn scan_tree(
    root: &File,
    path: &Utf8Path,
    limit: usize,
) -> Result<(Vec<FileScan>, bool), Refusal> {
    let mut frames = vec![Frame::new(
        root.try_clone().map_err(Refusal::Read)?,
        path.to_owned(),
    )?];
    let mut scans = Vec::new();
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
        let stat = fs::statat(&frame.directory, name.as_str(), AtFlags::SYMLINK_NOFOLLOW)
            .map_err(error)?;
        let kind = FileType::from_raw_mode(stat.st_mode);
        let child = frame.path.join(&name);
        if kind.is_symlink() {
            if child
                .extension()
                .and_then(Language::from_extension)
                .is_some()
            {
                return Err(Refusal::Denied);
            }
            continue;
        }
        if kind.is_dir() {
            let child_directory = directory(&frame.directory, &name)?;
            frames.push(Frame::new(child_directory, child)?);
        } else if let Some(language) = child.extension().and_then(Language::from_extension) {
            if !kind.is_file() {
                return Err(Refusal::Denied);
            }
            if scans.len() == limit {
                cut = true;
                break;
            }
            let mut file = regular(&frame.directory, &name)?;
            let mut content = String::new();
            file.read_to_string(&mut content).map_err(Refusal::Read)?;
            scans.push(provenance_scanner::scan_file(&child, language, &content));
        }
    }
    scans.sort_by(|a, b| a.file_path.cmp(&b.file_path));
    Ok((scans, cut))
}
