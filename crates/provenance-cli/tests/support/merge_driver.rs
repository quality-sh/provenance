//! A temporary Git repository with the JSONL merge driver configured for
//! its state files, and the commands that the merge tests run in it.

use assert_cmd::Command;
use serde_json::Value;
use std::path::Path;

pub fn cli(repo: &Path, args: &[&str]) -> std::process::Output {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(args)
        .args(["--repo", repo.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap()
}

pub fn json(repo: &Path, args: &[&str]) -> Value {
    let output = cli(repo, args);
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

pub fn git(repo: &Path, args: &[&str]) -> std::process::Output {
    provenance_store::git_process::git()
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap()
}

pub fn git_ok(repo: &Path, args: &[&str]) {
    let output = git(repo, args);
    assert!(
        output.status.success(),
        "git {args:?}: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn commit(repo: &Path, message: &str) {
    git_ok(repo, &["add", "."]);
    git_ok(repo, &["commit", "-m", message]);
}

/// A Git repository where Git merges each state file with the JSONL merge
/// driver of the built binary.
pub fn driver_repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git_ok(repo, &["init", "--initial-branch", "main"]);
    git_ok(repo, &["config", "user.email", "test@example.test"]);
    git_ok(repo, &["config", "user.name", "Test"]);
    let binary = assert_cmd::cargo::cargo_bin!("provenance");
    git_ok(
        repo,
        &[
            "config",
            "merge.provenance-jsonl.driver",
            &format!(
                "'{}' merge-jsonl %O %A %B --output %A --path %P",
                binary.to_str().unwrap().replace('\\', "/")
            ),
        ],
    );
    std::fs::write(
        repo.join(".gitattributes"),
        ".provenance/state/**/*.jsonl merge=provenance-jsonl\n",
    )
    .unwrap();
    dir
}

/// A repository with one scope where Git merges each state file with the
/// JSONL merge driver of the built binary.
pub fn repository() -> tempfile::TempDir {
    let dir = driver_repository();
    let repo = dir.path();
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
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
    dir
}
