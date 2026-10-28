use super::super::*;
use camino::Utf8Path;

fn repository() -> (tempfile::TempDir, camino::Utf8PathBuf, RepositoryFiles) {
    let temporary = tempfile::tempdir().unwrap();
    let path =
        camino::Utf8PathBuf::from_path_buf(temporary.path().canonicalize().unwrap()).unwrap();
    let files = RepositoryFiles::open(&path).unwrap();
    (temporary, path, files)
}

fn artifacts(path: &Utf8Path, suffix: &str) -> Vec<camino::Utf8PathBuf> {
    std::fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|entry| {
            entry
                .file_name()
                .unwrap()
                .to_string_lossy()
                .ends_with(suffix)
        })
        .filter_map(|entry| camino::Utf8PathBuf::from_path_buf(entry).ok())
        .collect()
}

#[test]
fn clone_failure_cleans_up_the_created_temporary_file() {
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"original").unwrap();
    let held = files.read_bounded(Utf8Path::new("source.txt"), 20).unwrap();
    crate::test_probes::arm("repository_file_clone_parent", || {
        anyhow::bail!("clone failed")
    });

    let result = held.create_temp(b"replacement");
    crate::test_probes::disarm("repository_file_clone_parent");

    assert!(matches!(result, Err(RepositoryFileRefusal::Write(_))));
    assert_eq!(artifacts(&path, ".tmp"), [] as [camino::Utf8PathBuf; 0]);
}

#[test]
fn temporary_unlink_failure_leaves_a_documented_artifact() {
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"original").unwrap();
    let held = files.read_bounded(Utf8Path::new("source.txt"), 20).unwrap();
    crate::test_probes::arm("repository_file_clone_parent", || {
        anyhow::bail!("clone failed")
    });
    crate::test_probes::arm("repository_file_unlink_temp", || {
        anyhow::bail!("unlink failed")
    });

    let result = held.create_temp(b"replacement");
    crate::test_probes::disarm("repository_file_clone_parent");
    crate::test_probes::disarm("repository_file_unlink_temp");

    assert!(matches!(result, Err(RepositoryFileRefusal::Write(_))));
    assert_eq!(artifacts(&path, ".tmp").len(), 1);
    assert_eq!(std::fs::read(path.join("source.txt")).unwrap(), b"original");
}

#[test]
fn backup_unlink_failure_reports_an_installed_replacement_and_named_backup() {
    let (_temporary, path, files) = repository();
    std::fs::write(path.join("source.txt"), b"original").unwrap();
    let held = files.read_bounded(Utf8Path::new("source.txt"), 20).unwrap();
    let prepared = held.create_temp(b"replacement").unwrap();
    crate::test_probes::arm("repository_file_unlink_backup", || {
        anyhow::bail!("unlink failed")
    });

    let outcome = held.compare_and_swap(prepared).unwrap();
    crate::test_probes::disarm("repository_file_unlink_backup");

    let RepositoryFileInstall::InstalledBackupKept { backup, reason } = outcome else {
        panic!("the failed cleanup must report its backup");
    };
    assert_eq!(reason, BackupRetentionReason::RemovalFailed);
    assert_eq!(std::fs::read(path.join(backup)).unwrap(), b"original");
    assert_eq!(
        std::fs::read(path.join("source.txt")).unwrap(),
        b"replacement"
    );
}
