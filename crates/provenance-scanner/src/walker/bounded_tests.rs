//! The bounded walk cuts the same files on every run and says when it
//! stopped; under the limit it answers what `scan_path` answers.

use super::{scan_path, scan_path_bounded};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::verifies;

/// Six language files across nested directories, beside files the
/// scanner never reads, written in an order that is not the sorted one.
fn tree() -> (tempfile::TempDir, Utf8PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    for (path, content) in [
        ("z/last.rs", "#[rule(\"rule_z\")]\nfn last() {}\n"),
        (
            "b/mid.ts",
            "// @provenance rule: rule_b\nexport function mid() {}\n",
        ),
        ("a/first.rs", "#[rule(\"rule_a\")]\nfn first() {}\n"),
        ("a/notes.md", "not scanned\n"),
        (
            "a/second.py",
            "# @provenance rule: rule_a2\ndef second():\n    pass\n",
        ),
        ("m.rs", "#[rule(\"rule_m\")]\nfn m() {}\n"),
        (
            "b/deep/inner.rs",
            "#[rule(\"rule_inner\")]\nfn inner() {}\n",
        ),
        (
            "target/skipped.rs",
            "#[rule(\"rule_skipped\")]\nfn skipped() {}\n",
        ),
    ] {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, content).unwrap();
    }
    (dir, root)
}

fn relative(root: &Utf8Path, files: &[super::FileScan]) -> Vec<String> {
    files
        .iter()
        .map(|scan| scan.file_path.strip_prefix(root).unwrap().to_string())
        .collect()
}

#[test]
#[verifies("rule_scan_cut_is_deterministic", examples)]
fn a_cut_scan_reads_the_same_files_twice() {
    let (_dir, root) = tree();
    let (first, cut) = scan_path_bounded(&root, 4).unwrap();
    assert!(cut, "six language files do not fit a limit of four");
    let (second, cut_again) = scan_path_bounded(&root, 4).unwrap();
    assert!(cut_again);
    assert_eq!(first, second);
    assert_eq!(
        relative(&root, &first),
        ["a/first.rs", "a/second.py", "b/deep/inner.rs", "b/mid.ts"],
        "the first four language files in sorted walk order; the note and the target tree never count"
    );
}

#[test]
#[verifies("rule_scan_cut_is_deterministic", examples)]
fn a_sub_limit_scan_matches_scan_path() {
    let (_dir, root) = tree();
    let whole = scan_path(&root).unwrap();
    assert_eq!(whole.len(), 6);
    for limit in [6, 7, usize::MAX] {
        let (bounded, cut) = scan_path_bounded(&root, limit).unwrap();
        assert!(!cut, "limit {limit}");
        assert_eq!(bounded, whole, "limit {limit}");
    }
    let (exact, cut) = scan_path_bounded(&root, 5).unwrap();
    assert!(cut, "the sixth file is met and the walk says so");
    assert_eq!(exact.len(), 5);
}
