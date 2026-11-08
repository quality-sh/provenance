use super::*;

fn git(root: &Utf8Path, args: &[&str]) -> String {
    let output = crate::git_process::git()
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn source(index: usize, body: u32) -> String {
    format!("#[rule(\"rule_{index}\")]\nfn item_{index}() -> u32 {{\n    {body}\n}}\n")
}

/// A base with more files than one thread parses, and a head that changes,
/// removes and adds a few of them.
fn range() -> (tempfile::TempDir, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    git(root, &["init", "-q"]);
    std::fs::create_dir_all(root.join("src")).unwrap();
    for index in 0..300 {
        std::fs::write(root.join(format!("src/f{index}.rs")), source(index, 1)).unwrap();
    }
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "Base"]);
    let base = git(root, &["rev-parse", "HEAD"]);
    std::fs::write(root.join("src/f7.rs"), source(7, 2)).unwrap();
    std::fs::remove_file(root.join("src/f8.rs")).unwrap();
    std::fs::write(root.join("src/new.rs"), source(900, 1)).unwrap();
    git(root, &["add", "-A", "."]);
    git(root, &["commit", "-qm", "Head"]);
    let head = git(root, &["rev-parse", "HEAD"]);
    (dir, base, head)
}

fn one_by_one(root: &Utf8Path, revision: &str) -> Vec<FileScanWithContent> {
    git::revision_files(root, revision)
        .unwrap()
        .into_iter()
        .map(gate::scan_file)
        .collect()
}

#[test]
fn range_scans_equal_the_scans_of_each_file_in_tree_order() {
    let (dir, base, head) = range();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let expected_base = one_by_one(root, &base);
    let expected_head = one_by_one(root, &head);
    assert_eq!(expected_base.len(), 300);

    for _ in 0..2 {
        let scans = scan_range(root, &base, &head).unwrap();
        assert_eq!(scans.base.files, expected_base);
        assert_eq!(scans.head.files, expected_head);
    }
    let same = scan_range(root, &head, &head).unwrap();
    assert!(Arc::ptr_eq(&same.base, &same.head));
    assert_eq!(same.head.files, expected_head);
    let reversed = scan_range(root, &head, &base).unwrap();
    assert_eq!(reversed.base.files, expected_head);
    assert_eq!(reversed.head.files, expected_base);
}

#[test]
fn a_failed_read_gives_the_git_failure() {
    let (dir, base, _) = range();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let error = scan_range(root, &base, "no-such-revision")
        .err()
        .expect("an unknown revision is refused");
    assert!(
        error
            .to_string()
            .starts_with("git ls-tree -rz no-such-revision failed"),
        "{error}"
    );
}

#[test]
fn parse_keeps_the_input_order_on_many_threads() {
    let files = (0..500)
        .map(|index| RevisionFile {
            path: Utf8PathBuf::from(format!("src/f{index}.rs")),
            content: source(index, 1),
        })
        .collect::<Vec<_>>();
    let expected = files
        .iter()
        .map(|file| {
            gate::scan_file(RevisionFile {
                path: file.path.clone(),
                content: file.content.clone(),
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(parse(files), expected);
}
