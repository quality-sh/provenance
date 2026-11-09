//! A git repository with a Provenance graph, for the stale query tests.
// Each test binary that includes this file uses only some of the helpers.
#![allow(dead_code)]

use crate::review_paths::{init_scope, provenance};
use serde_json::Value;
use std::path::Path;

/// Runs git in `repo` and gives the trimmed standard output.
pub fn git(repo: &Path, args: &[&str]) -> String {
    let output = provenance_store::git_process::git()
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

pub fn write(repo: &Path, relative: &str, contents: impl AsRef<[u8]>) {
    let path = repo.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

/// Commits every change in the work tree and gives the commit id.
pub fn commit(repo: &Path, message: &str) -> String {
    git(repo, &["add", "-A", "."]);
    git(repo, &["commit", "-q", "-m", message]);
    git(repo, &["rev-parse", "HEAD"])
}

/// A new git repository with one `default` scope at the root.
pub fn init_repo() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    git(directory.path(), &["init", "-q"]);
    git(
        directory.path(),
        &["config", "user.email", "test@example.com"],
    );
    git(directory.path(), &["config", "user.name", "Test"]);
    init_scope(directory.path(), "default", ".");
    directory
}

/// Applies one typed specification to the `default` scope.
pub fn apply_spec(repo: &Path, input: &Value) {
    provenance()
        .args([
            "api",
            "authoring-changes",
            "--method",
            "post",
            "--input",
            "-",
        ])
        .arg("--repo")
        .arg(repo)
        .args(["--scope", "default", "--format", "json"])
        .write_stdin(serde_json::to_vec(input).unwrap())
        .assert()
        .success();
}

/// Deletes the loose object of `object` from the object store.
pub fn remove_object(repo: &Path, object: &str) {
    std::fs::remove_file(
        repo.join(".git/objects")
            .join(&object[..2])
            .join(&object[2..]),
    )
    .unwrap();
}
