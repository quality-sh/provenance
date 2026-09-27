//! Prepared-file tests assert the artifacts that remain at each interruption point.
//!
//! Before displacement, a failure leaves the source and can leave the prepared file.
//! After displacement, a crash leaves the source name absent, plus the prepared file and
//! backup. After installation, a crash leaves the replacement and backup.

use super::super::*;
use camino::Utf8Path;

fn repository() -> (tempfile::TempDir, camino::Utf8PathBuf, RepositoryFiles) {
    let temporary = tempfile::tempdir().unwrap();
    let path = camino::Utf8PathBuf::from_path_buf(temporary.path().canonicalize().unwrap()).unwrap();
    let files = RepositoryFiles::open(&path).unwrap();
    (temporary, path, files)
}

fn prepared_leaf(path: &Utf8Path) -> camino::Utf8PathBuf {
    std::fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|entry| entry.file_name().unwrap().to_string_lossy().ends_with(".tmp"))
        .and_then(|entry| camino::Utf8PathBuf::from_path_buf(entry).ok())
        .unwrap()
}

#[test]
fn prepared_file_is_bound_to_the_file_that_created_it() {
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("a.txt"), b"a").unwrap();
    std::fs::write(path.join("b.txt"), b"b").unwrap();
    let a = files.read_bounded(Utf8Path::new("a.txt"), 10).unwrap();
    let b = files.read_bounded(Utf8Path::new("b.txt"), 10).unwrap();
    let prepared = a.create_temp(b"for a").unwrap();

    assert!(matches!(
        b.compare_and_swap(prepared),
        Err(RepositoryFileRefusal::Changed)
    ));
    assert_eq!(std::fs::read(path.join("b.txt")).unwrap(), b"b");
}

#[test]
fn replaced_prepared_regular_file_is_refused() {
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"original").unwrap();
    let held = files.read_bounded(Utf8Path::new("source.txt"), 20).unwrap();
    let prepared = held.create_temp(b"trusted").unwrap();
    let leaf = prepared_leaf(&path);
    std::fs::rename(&leaf, path.join("held-prepared")).unwrap();
    std::fs::write(&leaf, b"attacker").unwrap();

    assert!(matches!(
        held.compare_and_swap(prepared),
        Err(RepositoryFileRefusal::Changed | RepositoryFileRefusal::Denied)
    ));
    assert_eq!(std::fs::read(path.join("source.txt")).unwrap(), b"original");
}

#[test]
fn replaced_prepared_hard_link_is_refused() {
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"original").unwrap();
    std::fs::write(path.join("attacker"), b"attacker").unwrap();
    let held = files.read_bounded(Utf8Path::new("source.txt"), 20).unwrap();
    let prepared = held.create_temp(b"trusted").unwrap();
    let leaf = prepared_leaf(&path);
    std::fs::rename(&leaf, path.join("held-prepared")).unwrap();
    std::fs::hard_link(path.join("attacker"), &leaf).unwrap();

    assert!(matches!(
        held.compare_and_swap(prepared),
        Err(RepositoryFileRefusal::Changed | RepositoryFileRefusal::Denied)
    ));
    assert_eq!(std::fs::read(path.join("source.txt")).unwrap(), b"original");
}

#[cfg(unix)]
#[test]
fn replaced_prepared_symlink_is_refused() {
    use std::os::unix::fs::symlink;
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"original").unwrap();
    std::fs::write(path.join("attacker"), b"attacker").unwrap();
    let held = files.read_bounded(Utf8Path::new("source.txt"), 20).unwrap();
    let prepared = held.create_temp(b"trusted").unwrap();
    let leaf = prepared_leaf(&path);
    std::fs::rename(&leaf, path.join("held-prepared")).unwrap();
    symlink("attacker", &leaf).unwrap();

    assert!(matches!(
        held.compare_and_swap(prepared),
        Err(RepositoryFileRefusal::Changed | RepositoryFileRefusal::Denied)
    ));
    assert_eq!(std::fs::read(path.join("source.txt")).unwrap(), b"original");
}

#[cfg(windows)]
#[test]
fn replaced_prepared_symlink_is_refused() {
    use std::os::windows::fs::symlink_file;
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"original").unwrap();
    std::fs::write(path.join("attacker"), b"attacker").unwrap();
    let held = files.read_bounded(Utf8Path::new("source.txt"), 20).unwrap();
    let prepared = held.create_temp(b"trusted").unwrap();
    let leaf = prepared_leaf(&path);
    std::fs::rename(&leaf, path.join("held-prepared")).unwrap();
    symlink_file(path.join("attacker"), &leaf).unwrap();

    assert!(matches!(
        held.compare_and_swap(prepared),
        Err(RepositoryFileRefusal::Changed | RepositoryFileRefusal::Denied)
    ));
    assert_eq!(std::fs::read(path.join("source.txt")).unwrap(), b"original");
}

#[test]
fn replacing_a_hard_link_does_not_write_through_to_the_other_name() {
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("other.txt"), b"original").unwrap();
    std::fs::hard_link(path.join("other.txt"), path.join("source.txt")).unwrap();
    let held = files.read_bounded(Utf8Path::new("source.txt"), 20).unwrap();
    let prepared = held.create_temp(b"replacement").unwrap();

    held.compare_and_swap(prepared).unwrap();

    assert_eq!(std::fs::read(path.join("source.txt")).unwrap(), b"replacement");
    assert_eq!(std::fs::read(path.join("other.txt")).unwrap(), b"original");
}
