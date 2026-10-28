use super::*;

fn fixture() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let git = |args: &[&str]| {
        let output = Command::new("git")
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
    };
    git(&["init", "-q"]);
    std::fs::create_dir_all(root.join("src/with space")).unwrap();
    std::fs::write(root.join("src/a.rs"), "fn a() {}\n").unwrap();
    std::fs::write(root.join("src/empty.rs"), "").unwrap();
    std::fs::write(root.join("src/with space/b.py"), "def b():\n    pass\n").unwrap();
    std::fs::write(root.join("notes.txt"), "not source\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-qm", "Source"]);
    let head = git(&["rev-parse", "HEAD"]);
    (dir, head)
}

fn contents(files: Vec<RevisionFile>) -> Vec<(String, String)> {
    files
        .into_iter()
        .map(|file| (file.path.into_string(), file.content))
        .collect()
}

#[test]
fn revision_files_reads_every_source_blob_in_one_batch() {
    let (dir, head) = fixture();
    let root = Utf8Path::from_path(dir.path()).unwrap();

    let files = contents(revision_files(root, &head).unwrap());

    assert_eq!(
        files,
        vec![
            ("src/a.rs".to_owned(), "fn a() {}\n".to_owned()),
            ("src/empty.rs".to_owned(), String::new()),
            (
                "src/with space/b.py".to_owned(),
                "def b():\n    pass\n".to_owned()
            ),
        ]
    );
}

#[test]
fn revision_files_at_paths_reads_only_selected_blobs() {
    let (dir, head) = fixture();
    let root = Utf8Path::from_path(dir.path()).unwrap();
    let selected = BTreeSet::from([
        "src/with space/b.py".to_owned(),
        "src/missing.rs".to_owned(),
    ]);

    let files = contents(revision_files_at_paths(root, &head, &selected).unwrap());

    assert_eq!(
        files,
        vec![(
            "src/with space/b.py".to_owned(),
            "def b():\n    pass\n".to_owned()
        )]
    );
    assert!(revision_files_at_paths(root, &head, &BTreeSet::new())
        .unwrap()
        .is_empty());
}
