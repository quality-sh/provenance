//! A Git repository with one commit for the tests of the git adapter.

use camino::Utf8PathBuf;
use std::path::Path;

pub fn git(root: &Path, args: &[&str]) -> String {
    let output = provenance_store::git_process::git()
        .args(["-c", "user.name=Ann", "-c", "user.email=ann@example.com"])
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?} failed: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// A repository whose one commit holds each of `files` with its text. Gives
/// the root and the commit id.
pub fn repository(temp: &tempfile::TempDir, files: &[(&str, &str)]) -> (Utf8PathBuf, String) {
    repository_in(temp, &["init", "-q"], files)
}

/// A repository that `init` makes, whose one commit holds each of `files`
/// with its text. Gives the root and the commit id.
pub fn repository_in(
    temp: &tempfile::TempDir,
    init: &[&str],
    files: &[(&str, &str)],
) -> (Utf8PathBuf, String) {
    let root = temp.path();
    git(root, init);
    for (path, text) in files {
        std::fs::write(root.join(path), text).unwrap();
        git(root, &["add", path]);
    }
    git(root, &["commit", "-q", "-m", "files"]);
    let commit = git(root, &["rev-parse", "HEAD"]);
    let root = Utf8PathBuf::from_path_buf(dunce::canonicalize(root).unwrap()).unwrap();
    (root, commit)
}
