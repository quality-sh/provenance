use super::super::*;
use camino::Utf8Path;
use sha2::Digest as _;

fn repository() -> (tempfile::TempDir, camino::Utf8PathBuf, RepositoryFiles) {
    let temporary = tempfile::tempdir().unwrap();
    let path = camino::Utf8PathBuf::from_path_buf(temporary.path().canonicalize().unwrap()).unwrap();
    let files = RepositoryFiles::open(&path).unwrap();
    (temporary, path, files)
}

#[test]
fn bounded_read_and_replace_preserve_exact_utf8_bytes() {
    let (_temporary, path, files) = repository();
    let original = "first\r\nGrüße\r\n".as_bytes();
    let replacement = "second\r\nκόσμος\r\n".as_bytes();
    std::fs::write(path.join("source.txt"), original).unwrap();

    let held = files
        .read_bounded(Utf8Path::new("source.txt"), original.len())
        .unwrap();
    assert_eq!(held.bytes(), original);
    assert_eq!(held.digest(), sha2::Sha256::digest(original).into());

    let prepared = held.create_temp(replacement).unwrap();
    held.compare_and_swap(prepared).unwrap();
    assert_eq!(std::fs::read(path.join("source.txt")).unwrap(), replacement);
}

#[test]
fn bounded_read_refuses_a_file_above_the_limit() {
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"12345").unwrap();

    assert!(matches!(
        files.read_bounded(Utf8Path::new("source.txt"), 4),
        Err(RepositoryFileRefusal::TooLarge { limit: 4 })
    ));
}

#[test]
fn compare_and_swap_refuses_changed_bytes_and_preserves_them() {
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"original").unwrap();
    let held = files
        .read_bounded(Utf8Path::new("source.txt"), 100)
        .unwrap();
    let prepared = held.create_temp(b"replacement").unwrap();
    std::fs::write(path.join("source.txt"), b"third party").unwrap();

    assert!(matches!(
        held.compare_and_swap(prepared),
        Err(RepositoryFileRefusal::Changed)
    ));
    assert_eq!(
        std::fs::read(path.join("source.txt")).unwrap(),
        b"third party"
    );
}

#[cfg(unix)]
#[test]
fn held_write_refuses_a_symlink_leaf_and_parent() {
    use std::os::unix::fs::symlink;
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("actual.txt"), b"actual").unwrap();
    symlink("actual.txt", path.join("leaf.txt")).unwrap();
    std::fs::create_dir(path.join("actual-parent")).unwrap();
    std::fs::write(path.join("actual-parent/source.txt"), b"actual").unwrap();
    symlink("actual-parent", path.join("linked-parent")).unwrap();

    for selected in ["leaf.txt", "linked-parent/source.txt"] {
        assert!(matches!(
            files.read_bounded(Utf8Path::new(selected), 100),
            Err(RepositoryFileRefusal::Denied)
        ));
    }
}

#[test]
fn held_write_refuses_paths_outside_the_root() {
    let (_temporary, _path, files) = repository();
    for selected in ["../outside.txt", "/outside.txt"] {
        assert!(matches!(
            files.read_bounded(Utf8Path::new(selected), 100),
            Err(RepositoryFileRefusal::Denied)
        ));
    }
}

#[cfg(unix)]
#[test]
fn compare_and_swap_keeps_the_original_permissions() {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"original").unwrap();
    std::fs::set_permissions(
        path.join("source.txt"),
        std::fs::Permissions::from_mode(0o754),
    )
    .unwrap();
    let before = std::fs::metadata(path.join("source.txt")).unwrap();
    let held = files
        .read_bounded(Utf8Path::new("source.txt"), 100)
        .unwrap();
    let identity = held.identity().clone();
    let prepared = held.create_temp(b"replacement").unwrap();

    held.compare_and_swap(prepared).unwrap();

    let after = std::fs::metadata(path.join("source.txt")).unwrap();
    assert_eq!(after.mode() & 0o777, before.mode() & 0o777);
    assert_ne!(
        files
            .read_bounded(Utf8Path::new("source.txt"), 100)
            .unwrap()
            .identity(),
        &identity
    );
}
