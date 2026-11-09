//! Implementation aids: these tests check the private kept scans directly.
//! The public answers are checked through the CLI, HTTP and MCP in
//! `crates/provenance-cli/tests/cli_stale_answers.rs`.

use super::*;
use crate::stale::git::batch_tests::git;
use provenance_macros::verifies;

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

    let kept = KeptScans::new();
    for _ in 0..2 {
        let scans = scan_range(&kept, root, &base, &head).unwrap();
        assert_eq!(scans.base.files, expected_base);
        assert_eq!(scans.head.files, expected_head);
    }
    let same = scan_range(&kept, root, &head, &head).unwrap();
    assert!(Arc::ptr_eq(&same.base, &same.head));
    assert_eq!(same.head.files, expected_head);
    let reversed = scan_range(&kept, root, &head, &base).unwrap();
    assert_eq!(reversed.base.files, expected_head);
    assert_eq!(reversed.head.files, expected_base);
}

#[test]
fn a_failed_read_gives_the_git_failure() {
    let (dir, base, _) = range();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let error = scan_range(&KeptScans::new(), root, &base, "no-such-revision")
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

fn kept_count(kept: &KeptScans) -> usize {
    kept.lock().len()
}

#[test]
fn a_repeated_range_uses_the_kept_scans_and_a_new_range_replaces_them() {
    let (dir, base, head) = range();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let kept = KeptScans::new();
    let before = scan_range(&kept, root, &base, &head).unwrap();
    let after = scan_range(&kept, root, &base, &head).unwrap();
    assert!(Arc::ptr_eq(&before.base, &after.base));
    assert!(Arc::ptr_eq(&before.head, &after.head));
    assert_eq!(kept_count(&kept), 2);

    let same = scan_range(&kept, root, &head, &head).unwrap();
    assert!(Arc::ptr_eq(&same.head, &before.head));
    assert_eq!(kept_count(&kept), 1);
    let again = scan_range(&kept, root, &base, &head).unwrap();
    assert!(!Arc::ptr_eq(&again.base, &before.base));
    assert_eq!(again.base.files, before.base.files);
}

#[test]
#[verifies("rule_kept_scans_match_new_process", examples)]
fn clones_of_an_open_share_the_kept_scans_and_another_open_does_not() {
    use crate::layout::ProvenanceLayout;
    use crate::repository::Repository;
    let (dir, base, head) = range();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    crate::test_support::initialize(&ProvenanceLayout::new(root.as_str()), &[]);
    let opened = Repository::open(root, "default").unwrap();
    let repo = opened.working_tree().to_owned();
    let clone = opened.clone();
    let first = scan_range(clone.kept_scans(), &repo, &base, &head).unwrap();
    let shared = scan_range(opened.kept_scans(), &repo, &base, &head).unwrap();
    assert!(Arc::ptr_eq(&first.base, &shared.base));
    let other = Repository::open(root, "default").unwrap();
    assert_eq!(kept_count(other.kept_scans()), 0);
    let own = scan_range(other.kept_scans(), &repo, &base, &head).unwrap();
    assert!(!Arc::ptr_eq(&first.base, &own.base));
    assert_eq!(first.base.files, own.base.files);
}

#[test]
fn a_range_over_the_byte_limit_is_not_kept() {
    let (dir, base, head) = range();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let kept = KeptScans::with_limit(KEPT_BYTES);
    scan_range(&kept, root, &base, &head).unwrap();
    assert_eq!(kept_count(&kept), 2);
    let small = KeptScans::with_limit(1);
    scan_range(&small, root, &base, &head).unwrap();
    assert_eq!(kept_count(&small), 0);
}

#[test]
#[verifies("rule_kept_scans_match_new_process", examples)]
fn a_kept_range_gives_the_read_failure_of_a_missing_blob() {
    let (dir, base, head) = range();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let kept = KeptScans::new();
    scan_range(&kept, root, &base, &head).unwrap();
    let blob = git(root, &["rev-parse", &format!("{head}:src/new.rs")]);
    std::fs::remove_file(root.join(format!(".git/objects/{}/{}", &blob[..2], &blob[2..]))).unwrap();
    let cold = scan_range(&KeptScans::new(), root, &base, &head)
        .err()
        .expect("a new process cannot read the blob");
    let warm = scan_range(&kept, root, &base, &head)
        .err()
        .expect("kept scans give the same failure");
    assert_eq!(warm.to_string(), cold.to_string());
    assert!(cold.to_string().contains("src/new.rs"), "{cold}");
}
