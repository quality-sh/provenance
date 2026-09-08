use super::super::FileAccessRefusal;
use super::*;

fn tree() -> (tempfile::TempDir, camino::Utf8PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let path =
        camino::Utf8PathBuf::from_path_buf(temporary.path().canonicalize().unwrap()).unwrap();
    std::fs::create_dir(path.join("src")).unwrap();
    std::fs::write(
        path.join("src/a.rs"),
        "#[rule(\"rule_safe\")]\nfn safe() {}\n",
    )
    .unwrap();
    std::fs::write(
        path.join("z.ts"),
        "// @provenance rule: rule_last\nfunction last() {}\n",
    )
    .unwrap();
    (temporary, path)
}

#[test]
fn held_traversal_reads_files_and_preserves_bounded_scan_order() {
    let (_temporary, path) = tree();
    let root = open_root(&path).unwrap();
    let mut file = open_file(&root, Utf8Path::new("src/a.rs")).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    assert!(contents.contains("rule_safe"));
    let (scans, cut) = scan_tree(&root, &path, 1).unwrap();
    assert!(cut);
    assert_eq!(scans.len(), 1);
    assert_eq!(scans[0].file_path, path.join("src/a.rs"));
    let (complete, cut) = scan_tree(&root, &path, 2).unwrap();
    assert!(!cut);
    assert_eq!(complete.len(), 2);
}

#[cfg(unix)]
fn symlink_dir(target: &Utf8Path, link: &Utf8Path) {
    std::os::unix::fs::symlink(target, link).unwrap();
}
#[cfg(windows)]
fn symlink_dir(target: &Utf8Path, link: &Utf8Path) {
    std::os::windows::fs::symlink_dir(target, link).unwrap();
}

#[test]
fn held_traversal_refuses_replaced_ancestors_and_ignores_excluded_links() {
    let (_temporary, path) = tree();
    let (_outside, outside) = tree();
    let root = open_root(&path).unwrap();
    std::fs::rename(path.join("src"), path.join("old")).unwrap();
    symlink_dir(&outside.join("src"), &path.join("src"));
    assert!(matches!(
        open_file(&root, Utf8Path::new("src/a.rs")),
        Err(FileAccessRefusal::Denied)
    ));
    symlink_dir(&outside.join("src"), &path.join("alias.rs"));
    assert!(matches!(
        scan_tree(&root, &path, 10),
        Err(FileAccessRefusal::Denied)
    ));
    #[cfg(unix)]
    std::fs::remove_file(path.join("src")).unwrap();
    #[cfg(windows)]
    std::fs::remove_dir(path.join("src")).unwrap();
    #[cfg(unix)]
    std::fs::remove_file(path.join("alias.rs")).unwrap();
    #[cfg(windows)]
    std::fs::remove_dir(path.join("alias.rs")).unwrap();
    symlink_dir(&outside, &path.join("node_modules"));
    assert_eq!(scan_tree(&root, &path, 10).unwrap().0.len(), 2);
}

#[test]
fn held_traversal_keeps_opened_file_after_replacement() {
    let (_temporary, path) = tree();
    let root = open_root(&path).unwrap();
    let mut file = open_file(&root, Utf8Path::new("src/a.rs")).unwrap();
    std::fs::rename(path.join("src/a.rs"), path.join("src/old.rs")).unwrap();
    std::fs::write(path.join("src/a.rs"), "OUTSIDE_SENTINEL").unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    assert!(contents.contains("rule_safe"));
    assert!(!contents.contains("OUTSIDE_SENTINEL"));
}

#[cfg(unix)]
#[test]
fn non_source_files_do_not_require_content_read_access() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_temporary, path) = tree();
    std::fs::write(path.join("notes.txt"), "not scanned").unwrap();
    std::fs::set_permissions(path.join("notes.txt"), std::fs::Permissions::from_mode(0o0)).unwrap();
    let root = open_root(&path).unwrap();
    let result = scan_tree(&root, &path, 10);
    std::fs::set_permissions(
        path.join("notes.txt"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert_eq!(result.unwrap().0.len(), 2);
}

#[test]
fn tree_skips_unsupported_link_names_without_reading_their_targets() {
    let (_temporary, path) = tree();
    let (_outside, outside) = tree();
    std::fs::write(
        outside.join("src/a.rs"),
        "#[rule(\"outside\")]\nfn OUTSIDE_SENTINEL() {}\n",
    )
    .unwrap();
    symlink_dir(&outside, &path.join("installed-skill"));
    let root = open_root(&path).unwrap();
    let (scans, cut) = scan_tree(&root, &path, 10).unwrap();
    assert_eq!(scans.len(), 2);
    assert!(!cut);
    assert!(!scans
        .iter()
        .flat_map(|scan| &scan.bindings)
        .any(|binding| binding.rule_id == "outside"));
}
