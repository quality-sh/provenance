use super::super::*;
use camino::Utf8Path;
use std::io::{Seek as _, Write as _};

fn repository() -> (tempfile::TempDir, camino::Utf8PathBuf, RepositoryFiles) {
    let temporary = tempfile::tempdir().unwrap();
    let path = camino::Utf8PathBuf::from_path_buf(temporary.path().canonicalize().unwrap()).unwrap();
    let files = RepositoryFiles::open(&path).unwrap();
    (temporary, path, files)
}

#[test]
fn bounded_read_refuses_a_file_that_grows_during_the_read() {
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"1234").unwrap();
    let source = path.join("source.txt");
    crate::test_probes::arm("repository_file_after_metadata", move || {
        std::fs::OpenOptions::new()
            .append(true)
            .open(&source)?
            .write_all(b"5")?;
        Ok(())
    });

    let result = files.read_bounded(Utf8Path::new("source.txt"), 4);
    crate::test_probes::disarm("repository_file_after_metadata");

    assert!(matches!(result, Err(RepositoryFileRefusal::TooLarge { limit: 4 })));
}

#[test]
fn replacing_the_held_parent_after_read_refuses_without_touching_the_new_parent() {
    let (_temporary, path, files) = repository();
    std::fs::create_dir(path.join("dir")).unwrap();
    std::fs::write(path.join("dir/source.txt"), b"original").unwrap();
    let held = files.read_bounded(Utf8Path::new("dir/source.txt"), 20).unwrap();
    let prepared = held.create_temp(b"replacement").unwrap();
    std::fs::rename(path.join("dir"), path.join("held-dir")).unwrap();
    std::fs::create_dir(path.join("dir")).unwrap();
    std::fs::write(path.join("dir/source.txt"), b"outside").unwrap();

    assert!(held.compare_and_swap(prepared).is_err());
    assert_eq!(std::fs::read(path.join("dir/source.txt")).unwrap(), b"outside");
    assert_eq!(std::fs::read(path.join("held-dir/source.txt")).unwrap(), b"original");
}

#[test]
fn replacing_the_root_after_read_refuses_without_touching_the_new_root() {
    let outer = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(outer.path().join("repo")).unwrap();
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("source.txt"), b"original").unwrap();
    let files = RepositoryFiles::open(&root).unwrap();
    let held = files.read_bounded(Utf8Path::new("source.txt"), 20).unwrap();
    let prepared = held.create_temp(b"replacement").unwrap();
    std::fs::rename(&root, outer.path().join("held-root")).unwrap();
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("source.txt"), b"outside").unwrap();

    assert!(held.compare_and_swap(prepared).is_err());
    assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), b"outside");
    assert_eq!(
        std::fs::read(outer.path().join("held-root/source.txt")).unwrap(),
        b"original"
    );
}

#[test]
fn a_write_to_the_displaced_inode_is_kept_in_a_named_backup() {
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"original").unwrap();
    let mut original = std::fs::OpenOptions::new()
        .write(true)
        .open(path.join("source.txt"))
        .unwrap();
    let held = files.read_bounded(Utf8Path::new("source.txt"), 20).unwrap();
    let prepared = held.create_temp(b"replacement").unwrap();
    crate::test_probes::arm("repository_file_after_backup_check", move || {
        original.rewind()?;
        original.write_all(b"changed!")?;
        original.set_len(8)?;
        original.sync_all()?;
        Ok(())
    });

    let outcome = held.compare_and_swap(prepared).unwrap();
    crate::test_probes::disarm("repository_file_after_backup_check");

    let RepositoryFileInstall::InstalledBackupKept { backup, reason } = outcome else {
        panic!("the changed original must remain as a backup");
    };
    assert_eq!(reason, BackupRetentionReason::OriginalChanged);
    assert_eq!(std::fs::read(path.join(backup)).unwrap(), b"changed!");
    assert_eq!(std::fs::read(path.join("source.txt")).unwrap(), b"replacement");
}
