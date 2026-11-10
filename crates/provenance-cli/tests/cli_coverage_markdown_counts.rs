//! Implementation-aid test, without a Rule binding: no active Rule states the count
//! lines of the markdown scan report. It keeps a reader from taking the count of comment
//! annotations as a count of bindings (bead provenance-n41n).

use assert_cmd::Command;

#[test]
fn markdown_report_counts_bindings_apart_from_comment_annotations() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    let source_dir = repo.join("src");
    let output = repo.join("coverage.md");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(
        source_dir.join("lib.rs"),
        "#[rule(\"rule_login_lockout\")]\npub fn lock() {}\n\n#[verifies(\"rule_login_lockout\", examples)]\nfn lock_examples() {}\n",
    )
    .unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "check",
            "--bindings",
            "--repo",
            repo.to_str().unwrap(),
            "--path",
            source_dir.to_str().unwrap(),
            "--scope",
            "default",
            "--output-format",
            "markdown",
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();

    let markdown = std::fs::read_to_string(output).unwrap();
    assert!(
        markdown.contains("- Code bindings: 2\n- Comment annotations: 0\n"),
        "{markdown}"
    );
    assert!(!markdown.contains("Total annotations"), "{markdown}");
}
