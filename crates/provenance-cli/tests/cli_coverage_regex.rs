//! Two-file command regression for GitHub issue #442.
use assert_cmd::Command;
use serde_json::Value;
use std::path::Path;

fn scan(files: &[(&str, &str)]) -> Value {
    let repo = tempfile::tempdir().unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.path().to_str().unwrap(),
            "--scope",
            "demo",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    for (name, source) in files {
        let path = repo.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }
    scan_command(repo.path())
}

fn scan_command(repo: &Path) -> Value {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "coverage",
            "scan",
            "--repo",
            repo.to_str().unwrap(),
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "demo",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

#[test]
fn issue_442_finds_both_typescript_annotations() {
    let report = scan(&[
        (
            "src/a.ts",
            include_str!("fixtures/coverage_regex/src/a.ts.txt"),
        ),
        (
            "src/b.ts",
            include_str!("fixtures/coverage_regex/src/b.ts.txt"),
        ),
    ]);
    assert_eq!(report["total_annotations"], 2, "{report}");
    assert_eq!(report["warnings"], serde_json::json!([]), "{report}");
}
