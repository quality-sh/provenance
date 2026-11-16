//! The Windows rename of `write_synced_file_checked` with names that need a
//! long rename buffer, and the form of the paths in its step texts. These
//! tests have no Rule binding: they pin the store call that the project file
//! Rules of init use.
use super::write_synced_file_checked;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const PREFIX: &str = ".AGENTS.md.provenance-";

fn directory() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = dunce::canonicalize(temp.path()).unwrap();
    (temp, root)
}

fn names(folder: &Path) -> Vec<OsString> {
    let mut names: Vec<OsString> = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    names.sort();
    names
}

/// The count of UTF-16 units of `path`.
fn units(path: &Path) -> usize {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().count()
}

/// Replaces the file at `path`, which has old bytes, and shows that the
/// folder then holds only that name, with the new bytes.
fn assert_replaced(path: &Path) {
    let folder = path.parent().unwrap();
    std::fs::create_dir_all(folder).unwrap();
    std::fs::write(path, "old\n").unwrap();

    let replaced = write_synced_file_checked(path, b"new\n", PREFIX, None, || Ok(())).unwrap();

    replaced.unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"new\n");
    assert_eq!(names(folder), [path.file_name().unwrap()]);
}

#[test]
fn a_name_with_a_character_outside_the_basic_plane_is_replaced() {
    let (_temp, root) = directory();
    let name = "AGENTS-\u{1F600}.md";
    assert_eq!(name.encode_utf16().count(), name.chars().count() + 1);

    assert_replaced(&root.join(name));
}

#[test]
fn a_name_of_255_units_is_replaced() {
    let (_temp, root) = directory();
    let name = format!("{}.md", "a".repeat(252));
    assert_eq!(name.encode_utf16().count(), 255);

    assert_replaced(&root.join(name));
}

#[test]
fn a_path_longer_than_260_characters_is_replaced() {
    let (_temp, root) = directory();
    let path = root
        .join("d".repeat(100))
        .join("e".repeat(100))
        .join("f".repeat(100))
        .join("AGENTS.md");
    assert!(units(&path) > 260);

    assert_replaced(&path);
}

/// The text of a failed remove of an old temporary file names the file in
/// the form of the path of the caller, with no `\\?\` prefix.
#[test]
fn a_failed_remove_of_an_old_temporary_file_names_the_file_as_the_caller_does() {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ;
    let (_temp, root) = directory();
    let path = root.join("AGENTS.md");
    std::fs::write(&path, "old\n").unwrap();
    let stopped = root.join(format!("{PREFIX}abc123.tmp"));
    std::fs::write(&stopped, "stopped\n").unwrap();
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&stopped)
        .unwrap();

    let error = write_synced_file_checked(&path, b"new\n", PREFIX, None, || Ok(())).unwrap_err();

    drop(held);
    assert_eq!(
        format!("{error:#}"),
        format!(
            "remove {}: {}",
            stopped.display(),
            std::io::Error::from_raw_os_error(32)
        )
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"old\n");
}
