//! The refusal of `provenance discussions <word>` gives the command that reads the
//! Discussion with that ID and the commands that list Discussions.
//!
//! Implementation aid: no active Rule states this refusal text, because
//! `rule_cli_one_discussion_read_form` states the read form and not the refusal.

#[path = "support/bounded_process.rs"]
mod bounded_process;
use bounded_process::Bounded;
use std::path::Path;

/// The two commands that list Discussions, as the end of each refusal gives them.
const LIST: &str = "`provenance <record-id> discussions` to list the Discussions of one record, \
                    or `provenance discussions --status <status>` to list the Discussions \
                    with a status\n";

/// Runs the binary in `directory` and gives the exit code, the standard output and the
/// standard error.
fn run(directory: &Path, words: &[&str]) -> (Option<i32>, String, String) {
    let output = bounded_process::provenance()
        .current_dir(directory)
        .args(words)
        .bounded_output();
    (
        output.status.code(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

/// Runs the binary in `directory` and gives the standard output of a successful command.
fn success(directory: &Path, words: &[&str]) -> String {
    let (code, stdout, stderr) = run(directory, words);
    assert_eq!(code, Some(0), "{words:?}: {stderr}");
    stdout
}

fn assert_word_refusal(directory: &Path, word: &str, read: &str) {
    assert_eq!(
        run(directory, &["discussions", word]),
        (
            Some(1),
            String::new(),
            format!(
                "Error: `provenance discussions` does not take a Discussion ID. Use {read}, \
                 {LIST}"
            )
        ),
        "{word:?}"
    );
}

#[test]
fn discussions_word_refusal_gives_the_read_command_of_the_word() {
    let directory = tempfile::tempdir().unwrap();
    assert_word_refusal(
        directory.path(),
        "open",
        "`provenance open discussion` to read the Discussion `open`",
    );
}

#[test]
fn discussions_empty_word_refusal_gives_the_general_forms() {
    let directory = tempfile::tempdir().unwrap();
    assert_word_refusal(
        directory.path(),
        "",
        "`provenance <discussion-id> discussion` to read one Discussion",
    );
}

#[test]
fn discussions_id_refusal_gives_a_command_that_reads_the_discussion() {
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
    let start = success(
        repo,
        &[
            "req_a",
            "discuss",
            "--body",
            "Opening text",
            "--format",
            "json",
        ],
    );
    let start: serde_json::Value = serde_json::from_str(&start).unwrap();
    let id = start["receipt"]["discussion_id"].as_str().unwrap();
    assert_word_refusal(
        repo,
        id,
        &format!("`provenance {id} discussion` to read the Discussion `{id}`"),
    );
    let read = success(repo, &[id, "discussion"]);
    assert!(read.contains("Opening text"), "{read}");
}
