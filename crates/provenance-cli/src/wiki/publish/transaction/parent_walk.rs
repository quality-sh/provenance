//! The no-follow walk that opens, and creates, the wiki output parent.
use super::ownership::{open_child_directory_no_follow, open_directory_no_follow};
use crate::wiki::publish::PublishError;
use camino::Utf8Path;
use std::fs::File;
use std::iter::Peekable;
use std::path::{Component, Components, Path, PathBuf};

/// Resolve OS-level symlinks in the existing part of the parent path once,
/// before the no-follow descent. On macOS the default temp area lives under
/// `/var -> /private/var`, so refusing symlinked ancestors outright would
/// reject every standard temp path. After this resolution the descent still
/// refuses to follow symlinks, so a swap mid-walk fails closed.
fn resolve_existing_parent_prefix(parent: &Path) -> std::io::Result<PathBuf> {
    let mut existing = parent;
    let mut created_below = Vec::new();
    while !existing.try_exists()? {
        match (existing.file_name(), existing.parent()) {
            (Some(leaf), Some(next)) => {
                created_below.push(leaf.to_os_string());
                existing = next;
            }
            _ => break,
        }
    }
    let mut resolved = if existing.as_os_str().is_empty() {
        std::fs::canonicalize(".")?
    } else {
        std::fs::canonicalize(existing)?
    };
    #[cfg(windows)]
    {
        resolved = normalize_verbatim_prefix_for_walk(&resolved);
    }
    for leaf in created_below.iter().rev() {
        resolved.push(leaf);
    }
    Ok(resolved)
}

#[cfg(windows)]
fn normalize_verbatim_prefix_for_walk(path: &Path) -> PathBuf {
    use std::ffi::OsString;
    use std::path::Prefix;

    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return path.to_path_buf();
    };
    let mut normalized = match prefix.kind() {
        Prefix::VerbatimDisk(drive) => PathBuf::from(format!("{}:", char::from(drive))),
        Prefix::VerbatimUNC(server, share) => {
            let mut root = OsString::from(r"\\");
            root.push(server);
            root.push(r"\");
            root.push(share);
            PathBuf::from(root)
        }
        _ => return path.to_path_buf(),
    };
    normalized.extend(components);
    normalized
}

/// Opens the output parent one directory at a time without following
/// symlinks, and creates each missing directory on the way.
pub(super) fn open_or_create_parent(
    parent: &Utf8Path,
    output: &Utf8Path,
) -> Result<File, PublishError> {
    let parent_resolved = resolve_existing_parent_prefix(parent.as_std_path())
        .map_err(|error| PublishError::io("resolve output parent", parent, error))?;
    let mut components = parent_resolved.components().peekable();
    let mut current_path = PathBuf::new();
    let mut current = open_walk_root(&mut components, &mut current_path, parent, output)?;
    for component in components {
        let Some(leaf) = walk_leaf(component, output)? else {
            continue;
        };
        current_path.push(leaf);
        current = open_or_create_child(&current, leaf, &current_path, output)?;
    }
    Ok(current)
}

/// Opens the directory where the walk starts: the drive root, the root, or
/// the current directory. It consumes the root components.
fn open_walk_root(
    components: &mut Peekable<Components<'_>>,
    current_path: &mut PathBuf,
    parent: &Utf8Path,
    output: &Utf8Path,
) -> Result<File, PublishError> {
    let (start, operation) = match components.peek().copied() {
        Some(Component::Prefix(prefix)) => {
            components.next();
            current_path.push(prefix.as_os_str());
            if components.next_if_eq(&Component::RootDir).is_none() {
                return Err(invalid_output(
                    output,
                    "drive-relative output paths are unsupported".to_string(),
                ));
            }
            (std::path::MAIN_SEPARATOR_STR, "open output parent root")
        }
        Some(Component::RootDir) => {
            components.next();
            (std::path::MAIN_SEPARATOR_STR, "open output parent root")
        }
        _ => (".", "open current directory"),
    };
    current_path.push(start);
    open_directory_no_follow(current_path)
        .map_err(|error| PublishError::io(operation, parent, error))
}

/// The directory name that one parent component names, or `None` for a
/// component that the walk skips.
fn walk_leaf<'a>(
    component: Component<'a>,
    output: &Utf8Path,
) -> Result<Option<&'a str>, PublishError> {
    match component {
        Component::CurDir => Ok(None),
        Component::ParentDir => Ok(Some("..")),
        Component::Normal(leaf) => Ok(Some(leaf.to_str().expect("UTF-8 output parent component"))),
        Component::Prefix(_) | Component::RootDir => Err(invalid_output(
            output,
            "output parent contains an invalid path component".to_string(),
        )),
    }
}

/// Opens the child directory `leaf`, and creates it first when it is
/// missing. A child that is not a real directory refuses the walk.
fn open_or_create_child(
    current: &File,
    leaf: &str,
    current_path: &Path,
    output: &Utf8Path,
) -> Result<File, PublishError> {
    match open_child_directory_no_follow(current, leaf) {
        Ok(next) => Ok(next),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            create_child_directory(current, leaf, current_path)?;
            open_child_directory_no_follow(current, leaf).map_err(|error| {
                PublishError::io(
                    "open created output parent directory",
                    utf8(current_path),
                    error,
                )
            })
        }
        Err(_) => Err(invalid_output(
            output,
            format!(
                "output parent ancestor {} must be a real directory",
                current_path.display()
            ),
        )),
    }
}

/// Creates the child directory `leaf`. A directory that another writer
/// created first is not an error.
fn create_child_directory(
    current: &File,
    leaf: &str,
    current_path: &Path,
) -> Result<(), PublishError> {
    match fs_at::OpenOptions::default().mkdir_at(current, leaf) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(PublishError::io(
            "create output parent directory",
            utf8(current_path),
            error,
        )),
    }
}

fn invalid_output(output: &Utf8Path, detail: String) -> PublishError {
    PublishError::InvalidOutputPath {
        path: output.to_path_buf(),
        detail,
    }
}

fn utf8(path: &Path) -> &Utf8Path {
    Utf8Path::from_path(path).expect("UTF-8 output parent")
}
