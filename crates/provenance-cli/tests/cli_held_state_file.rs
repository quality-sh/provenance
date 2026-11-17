//! A write of records while a different process has a record file open,
//! as an editor, a virus scanner or a second command can have it. On Linux
//! and macOS a rename always replaces an open file, so the held-file tests
//! can fail only on Windows.

use crate::bounded_process;
use bounded_process::Bounded;
use provenance_macros::verifies;
use std::path::{Path, PathBuf};

/// Runs the binary in `directory` and gives the standard output of a
/// successful command.
fn success(directory: &Path, words: &[&str]) -> String {
    let output = bounded_process::provenance()
        .current_dir(directory)
        .args(words)
        .bounded_output();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(output.status.code(), Some(0), "{words:?}: {stderr}");
    String::from_utf8(output.stdout).unwrap()
}

/// Starts a Discussion on `req_a` and gives its ID.
fn discuss(repo: &Path, body: &str) -> String {
    let start = success(
        repo,
        &["req_a", "discuss", "--body", body, "--format", "json"],
    );
    let start: serde_json::Value = serde_json::from_str(&start).unwrap();
    start["receipt"]["discussion_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Each file below `directory`.
fn files_below(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(files_below(&path));
        } else {
            files.push(path);
        }
    }
    files
}

/// A repository with one Discussion, so the Thread and Message shards
/// exist.
fn repository_with_a_discussion() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    success(repo, &["init", "--scope", "default", "--path-prefix", "."]);
    success(
        repo,
        &[
            "req_a",
            "create",
            "--type",
            "requirement",
            "--statement",
            "A discussion has one parent.",
        ],
    );
    discuss(repo, "First text");
    directory
}

/// The test keeps each Thread and Message shard open with the sharing of
/// `std::fs`, which lets other handles delete the file, while the CLI
/// starts a second Discussion. That write changes the Thread shard and the
/// Message shard, so it lands as one whole set.
#[test]
#[verifies("rule_write_passes_while_a_record_file_is_held", examples)]
fn a_new_discussion_passes_while_a_reader_has_the_discussion_files_open() {
    let directory = repository_with_a_discussion();
    let repo = directory.path();
    let threads = repo.join(".provenance/state/scopes/default/threads");
    let held: Vec<_> = files_below(&threads)
        .iter()
        .map(|path| std::fs::File::open(path).unwrap())
        .collect();
    assert!(held.len() >= 2, "the Thread and Message shards exist");

    let id = discuss(repo, "Second text");
    drop(held);

    let read = success(repo, &[&id, "discussion"]);
    assert!(read.contains("Second text"), "{read}");
}
