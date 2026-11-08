//! Implementation aids: these tests check the private kept scans directly.
//! The public answers are checked through the CLI, HTTP and MCP in
//! `crates/provenance-cli/tests/cli_stale_answers.rs`.

use super::*;
use crate::stale::git::batch_tests::git;

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

fn kept(max_repositories: usize, max_bytes: usize) -> Mutex<KeptScans> {
    Mutex::new(KeptScans::new(max_repositories, max_bytes))
}

#[test]
fn each_repository_keeps_its_own_range() {
    let (first_dir, first_base, first_head) = range();
    let (second_dir, second_base, second_head) = range();
    let first = Utf8Path::from_path(first_dir.path()).unwrap();
    let second = Utf8Path::from_path(second_dir.path()).unwrap();
    let scans = kept(KEPT_REPOSITORIES, KEPT_BYTES);
    let before = scan_range_with(&scans, first, &first_base, &first_head).unwrap();
    let other = scan_range_with(&scans, second, &second_base, &second_head).unwrap();
    let after = scan_range_with(&scans, first, &first_base, &first_head).unwrap();
    assert!(Arc::ptr_eq(&before.base, &after.base));
    assert!(Arc::ptr_eq(&before.head, &after.head));
    assert!(!Arc::ptr_eq(&before.base, &other.base));
    assert_eq!(lock(&scans).repositories.len(), 2);

    // With room for one repository, the other repository evicts the range,
    // and the answer is the same.
    let scans = kept(1, KEPT_BYTES);
    let before = scan_range_with(&scans, first, &first_base, &first_head).unwrap();
    scan_range_with(&scans, second, &second_base, &second_head).unwrap();
    let after = scan_range_with(&scans, first, &first_base, &first_head).unwrap();
    assert!(!Arc::ptr_eq(&before.base, &after.base));
    assert_eq!(before.base.files, after.base.files);
    assert_eq!(before.head.files, after.head.files);
}

#[test]
fn a_range_over_the_byte_limit_is_not_kept() {
    let (dir, base, head) = range();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let scans = kept(KEPT_REPOSITORIES, 1);
    scan_range_with(&scans, root, &base, &head).unwrap();
    assert!(lock(&scans).repositories.is_empty());
}

#[test]
fn a_kept_range_gives_the_read_failure_of_a_missing_blob() {
    let (dir, base, head) = range();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let scans = kept(KEPT_REPOSITORIES, KEPT_BYTES);
    scan_range_with(&scans, root, &base, &head).unwrap();
    let blob = git(root, &["rev-parse", &format!("{head}:src/new.rs")]);
    std::fs::remove_file(root.join(format!(".git/objects/{}/{}", &blob[..2], &blob[2..]))).unwrap();
    let cold = scan_range_with(&kept(KEPT_REPOSITORIES, KEPT_BYTES), root, &base, &head)
        .err()
        .expect("a new process cannot read the blob");
    let warm = scan_range_with(&scans, root, &base, &head)
        .err()
        .expect("kept scans give the same failure");
    assert_eq!(warm.to_string(), cold.to_string());
    assert!(cold.to_string().contains("src/new.rs"), "{cold}");
}
