use super::*;
#[cfg(unix)]
use std::io::Read;

#[test]
fn portable_relative_identity_rejects_all_escape_forms() {
    for name in [
        "",
        "/abs.rs",
        "../x.rs",
        "a/../x.rs",
        "./x.rs",
        "a//x.rs",
        "C:x.rs",
        "a\\x.rs",
        "//server/file.rs",
    ] {
        assert!(validate_relative(Utf8Path::new(name)).is_err(), "{name}");
    }
    assert!(validate_relative(Utf8Path::new("src/file.rs")).is_ok());
}

#[cfg(unix)]
#[test]
fn opened_file_stays_bound_after_path_and_root_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Utf8Path::from_path(dir.path()).unwrap().join("repo");
    std::fs::create_dir(&repo).unwrap();
    std::fs::write(repo.join("safe.rs"), "safe").unwrap();
    let root = RepositoryFiles::open(&repo).unwrap();
    let mut opened = root.open_file(Utf8Path::new("safe.rs")).unwrap();
    std::fs::rename(&repo, repo.with_file_name("old")).unwrap();
    std::fs::create_dir(&repo).unwrap();
    std::fs::write(repo.join("safe.rs"), "OUTSIDE_SENTINEL").unwrap();
    let mut text = String::new();
    opened.file.read_to_string(&mut text).unwrap();
    assert_eq!(text, "safe");
    assert_eq!(opened.relative, "safe.rs");
    let mut again = String::new();
    root.open_file(Utf8Path::new("safe.rs"))
        .unwrap()
        .file
        .read_to_string(&mut again)
        .unwrap();
    assert_eq!(again, "safe");
}

#[cfg(unix)]
#[test]
fn nofollow_traversal_refuses_replaced_ancestors_and_special_files() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let path = Utf8Path::from_path(dir.path()).unwrap();
    std::fs::create_dir(path.join("src")).unwrap();
    std::fs::write(outside.path().join("sentinel.rs"), "OUTSIDE_SENTINEL").unwrap();
    let root = RepositoryFiles::open(path).unwrap();
    std::fs::remove_dir(path.join("src")).unwrap();
    symlink(outside.path(), path.join("src")).unwrap();
    assert!(matches!(
        root.open_file(Utf8Path::new("src/sentinel.rs")),
        Err(FileAccessRefusal::Denied)
    ));
    assert!(std::process::Command::new("mkfifo")
        .arg(path.join("pipe.rs"))
        .status()
        .unwrap()
        .success());
    assert!(matches!(
        root.open_file(Utf8Path::new("pipe.rs")),
        Err(FileAccessRefusal::Denied)
    ));
    assert!(root.scan_tree(10).is_err());
}

#[cfg(not(any(unix, windows)))]
#[test]
fn unsupported_platform_refuses_instead_of_reopening_a_checked_path() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        RepositoryFiles::open(Utf8Path::from_path(dir.path()).unwrap()),
        Err(FileAccessRefusal::Unavailable)
    ));
}

#[cfg(unix)]
#[test]
fn ignored_symlinks_are_not_entered_and_cuts_count_only_source_files() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let path = Utf8Path::from_path(dir.path()).unwrap();
    std::fs::write(
        outside.path().join("sentinel.rs"),
        "#[rule(\"outside\")]\nfn OUTSIDE_SENTINEL() {}\n",
    )
    .unwrap();
    for name in [".git", "node_modules", "target"] {
        std::os::unix::fs::symlink(outside.path(), path.join(name)).unwrap();
    }
    std::fs::write(path.join("a.rs"), "fn safe() {}\n").unwrap();
    std::fs::write(path.join("b.txt"), "ignored").unwrap();
    let root = RepositoryFiles::open(path).unwrap();
    let (scans, cut) = root.scan_tree(1).unwrap();
    assert_eq!(scans.len(), 1);
    assert!(!cut);
    assert!(scans[0].bindings.is_empty());
    assert!(root.scan_tree(0).unwrap().1);
}

#[cfg(unix)]
#[test]
fn selected_ancestor_replacement_never_reads_the_external_tree() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let root_path = Utf8Path::from_path(dir.path()).unwrap();
    std::fs::create_dir(root_path.join("src")).unwrap();
    std::fs::write(root_path.join("src/code.rs"), "safe").unwrap();
    std::fs::write(outside.path().join("code.rs"), "OUTSIDE_SENTINEL").unwrap();
    let root = RepositoryFiles::open(root_path).unwrap();
    let parent = root_path.to_owned();
    let outside_path = outside.path().to_owned();
    std::thread::scope(|threads| {
        threads.spawn(move || {
            for _ in 0..200 {
                std::fs::rename(parent.join("src"), parent.join("held")).unwrap();
                std::os::unix::fs::symlink(&outside_path, parent.join("src")).unwrap();
                std::fs::remove_file(parent.join("src")).unwrap();
                std::fs::rename(parent.join("held"), parent.join("src")).unwrap();
            }
        });
        for _ in 0..500 {
            if let Ok(mut opened) = root.open_file(Utf8Path::new("src/code.rs")) {
                let mut content = String::new();
                opened.file.read_to_string(&mut content).unwrap();
                assert_eq!(content, "safe");
            }
        }
    });
}

#[cfg(unix)]
#[test]
fn unscanned_symlink_entries_are_skipped_without_following_their_targets() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let root_path = Utf8Path::from_path(dir.path()).unwrap();
    std::fs::write(
        outside.path().join("sentinel.rs"),
        "#[rule(\"outside\")]\nfn OUTSIDE_SENTINEL() {}\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(outside.path(), root_path.join("skills")).unwrap();
    let root = RepositoryFiles::open(root_path).unwrap();
    let (scans, cut) = root.scan_tree(10).unwrap();
    assert!(scans.is_empty());
    assert!(!cut);
    assert!(matches!(
        root.open_file(Utf8Path::new("skills/sentinel.rs")),
        Err(FileAccessRefusal::Denied)
    ));
}
