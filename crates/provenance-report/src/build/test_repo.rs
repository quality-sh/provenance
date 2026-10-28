//! Git repository helpers for the report build tests. The integration tests
//! include this file by path, so the crate keeps one copy.

use std::path::Path;
use std::process::Command;

/// Run one Git command in `repo` and return its trimmed output.
pub fn git(repo: &(impl AsRef<Path> + ?Sized), args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repo)
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

/// Make `repo` an empty repository with a test identity.
pub fn init(repo: &(impl AsRef<Path> + ?Sized)) {
    git(repo, &["init", "-q"]);
    git(repo, &["config", "user.name", "Test"]);
    git(repo, &["config", "user.email", "test@example.com"]);
}

/// Commit every change in `repo`, also when there is no change, and return
/// the commit id.
pub fn commit(repo: &(impl AsRef<Path> + ?Sized), message: &str) -> String {
    git(repo, &["add", "."]);
    git(
        repo,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-qm",
            message,
        ],
    );
    git(repo, &["rev-parse", "HEAD"])
}
