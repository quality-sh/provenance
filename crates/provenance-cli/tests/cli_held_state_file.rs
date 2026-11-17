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

/// The refusal object of a failed command on standard error.
#[cfg(windows)]
fn refusal(repo: &Path, words: &[&str]) -> serde_json::Value {
    let output = bounded_process::provenance()
        .current_dir(repo)
        .args(words)
        .bounded_output();
    let text = String::from_utf8(output.stderr).unwrap();
    assert_eq!(output.status.code(), Some(1), "{words:?}: {text}");
    let line = text.lines().next().unwrap_or_default();
    let object = line.strip_prefix("Error: ").unwrap_or(line);
    serde_json::from_str(object).unwrap_or_else(|error| panic!("{words:?}: {error}: {text}"))
}

/// The test keeps the Message shard open with a share mode that lets other
/// handles only read, as a program that locks a file it reads does. The
/// write of a second Discussion succeeds, because its marker names all of
/// its changes. A read cannot complete the install while the file is held,
/// so it answers `state_file_in_use` with the file. When the holder closes
/// the file, the next read completes the write and shows the message one
/// time.
#[cfg(windows)]
#[test]
#[verifies("rule_held_record_file_refusal_names_the_file", examples)]
fn a_read_names_the_state_file_that_a_holder_with_no_delete_sharing_keeps() {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 0x1;
    let directory = repository_with_a_discussion();
    let repo = directory.path();
    let threads = repo.join(".provenance/state/scopes/default/threads");
    let shard = std::fs::read_dir(&threads)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .find(|name| name.as_bytes().first().is_some_and(u8::is_ascii_digit))
        .expect("the Message shard of the month exists");
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(threads.join(&shard))
        .unwrap();

    let id = discuss(repo, "Second text");
    let refused = refusal(repo, &["api", "/requirements/req_a"]);
    drop(held);

    assert_eq!(
        refused["error"],
        serde_json::json!({
            "kind": "state_file_in_use",
            "file": format!(".provenance/state/scopes/default/threads/{shard}")
        })
    );
    let read = success(repo, &[&id, "discussion"]);
    assert_eq!(read.matches("Second text").count(), 1, "{read}");
}
