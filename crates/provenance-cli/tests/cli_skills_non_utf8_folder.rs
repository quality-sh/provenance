//! `provenance skills install` in a folder whose path is not UTF-8. Linux
//! keeps a file name as bytes, so a name can hold a byte that is not UTF-8.
//! Windows keeps a file name as UTF-16 units, so a name can hold a surrogate
//! with no pair, which is not UTF-8. macOS refuses a file name that is not
//! UTF-8, so these tests do not run there.
#![cfg(any(target_os = "linux", windows))]

use assert_cmd::Command;
use std::path::{Path, PathBuf};

const SKILL: &str = "provenance-fork-tournament";

/// A new folder in `parent` whose name has the byte 0xFF, which is not UTF-8.
#[cfg(target_os = "linux")]
fn non_utf8_folder(parent: &Path) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    make_non_utf8_folder(parent.join(std::ffi::OsStr::from_bytes(b"skills-\xff")))
}

/// A new folder in `parent` whose name has the surrogate 0xD800 with no pair,
/// which is not UTF-8.
#[cfg(windows)]
fn non_utf8_folder(parent: &Path) -> PathBuf {
    use std::os::windows::ffi::OsStringExt;
    let units: Vec<u16> = "skills-".encode_utf16().chain([0xD800]).collect();
    make_non_utf8_folder(parent.join(std::ffi::OsString::from_wide(&units)))
}

fn make_non_utf8_folder(folder: PathBuf) -> PathBuf {
    std::fs::create_dir(&folder).unwrap();
    assert!(folder.to_str().is_none());
    folder
}

fn assert_installed(root: &Path) {
    let canonical = root.join(".agents/skills").join(SKILL).join("SKILL.md");
    let text = std::fs::read_to_string(&canonical).unwrap();
    assert!(
        text.starts_with("---\nname: provenance-fork-tournament"),
        "{text}"
    );
    let claude = root.join(".claude/skills").join(SKILL).join("SKILL.md");
    assert_eq!(std::fs::read_to_string(claude).unwrap(), text);
}

/// The install writes the skill files in the current folder, also when its
/// path is not UTF-8.
/// This test has no Rule binding: it checks that the replacement of a file
/// takes each path that the system gives.
#[test]
fn skills_install_writes_the_files_in_a_folder_that_is_not_utf8() {
    let directory = tempfile::tempdir().unwrap();
    let folder = non_utf8_folder(directory.path());
    let home = tempfile::tempdir().unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(&folder)
        .env("HOME", home.path())
        .args(["skills", "install", "--format", "json"])
        .assert()
        .code(0)
        .stdout(predicates::str::contains(r#""status": "installed""#));

    assert_installed(&folder);
}

/// The global install writes the skill files in the home folder, also when
/// its path is not UTF-8.
/// This test has no Rule binding: it checks that the replacement of a file
/// takes each path that the system gives.
#[test]
fn skills_install_global_writes_the_files_in_a_home_folder_that_is_not_utf8() {
    let directory = tempfile::tempdir().unwrap();
    let home = non_utf8_folder(directory.path());
    let current = tempfile::tempdir().unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(current.path())
        .env("HOME", &home)
        .args(["skills", "install", "--global", "--format", "json"])
        .assert()
        .code(0)
        .stdout(predicates::str::contains(r#""global": true"#));

    assert_installed(&home);
    assert!(!current.path().join(".agents").exists());
}
