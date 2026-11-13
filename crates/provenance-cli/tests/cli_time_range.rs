//! A time field past the range of the read database, ±(2^53 − 1), in a
//! committed record fails as a malformed field fails: `check` and a read
//! refuse with the file and the line of the record, and a read gives
//! `invalid_state_file`.
//!
//! No Rule: the plan proposes the Rule to the owner. Until it is in the
//! graph, this test holds the refusal.

use serde_json::Value;
use std::path::{Path, PathBuf};

mod support {
    pub mod review_paths;
}
use support::review_paths::{create_requirement, provenance};

/// The one Resolution file of the repository.
fn resolution_file(root: &Path) -> PathBuf {
    let directory = root.join(".provenance/state/scopes/default/resolutions");
    let mut files: Vec<PathBuf> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1, "{files:?}");
    files.remove(0)
}

/// A repository with one Resolution whose `approved_at` holds `value`.
fn repository(value: Value) -> (tempfile::TempDir, String) {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    provenance()
        .args([
            "resolution_a",
            "create",
            "--type",
            "resolution",
            "--status",
            "approved",
            "--repo",
            &repo,
            "--title",
            "Decision",
            "--requirement-id",
            "req_a",
            "--position",
            "Use the award threshold.",
            "--rationale",
            "The award clause sets the threshold.",
        ])
        .assert()
        .success();
    let path = resolution_file(directory.path());
    let text = std::fs::read_to_string(&path).unwrap();
    let mut record: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    record["approved_at"] = value;
    std::fs::write(&path, record.to_string() + "\n").unwrap();
    (directory, repo)
}

/// The text of `output` with the parse message of each line left out: the
/// value of `"message":"..."` in a JSON line, and the text after `line 1: `
/// in a text line.
fn without_messages(output: &[u8]) -> String {
    const MESSAGE: &str = "\"message\":\"";
    const LINE: &str = " line 1: ";
    String::from_utf8_lossy(output)
        .lines()
        .map(|line| match (line.find(MESSAGE), line.find(LINE)) {
            (Some(at), _) => {
                let start = at + MESSAGE.len();
                let rest = &line[start..];
                let mut escaped = false;
                let end = rest
                    .char_indices()
                    .find(|&(_, c)| {
                        let close = !escaped && c == '"';
                        escaped = !escaped && c == '\\';
                        close
                    })
                    .map_or(rest.len(), |(index, _)| index);
                format!("{}{}", &line[..start], &rest[end..])
            }
            (None, Some(at)) => line[..at + LINE.len()].to_owned(),
            (None, None) => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The exit code, standard output and standard error of `args` over `repo`.
fn run(repo: &str, args: &[&str]) -> (Option<i32>, Vec<u8>, Vec<u8>) {
    let output = provenance()
        .args(args)
        .args(["--repo", repo])
        .output()
        .unwrap();
    (output.status.code(), output.stdout, output.stderr)
}

#[test]
fn a_committed_time_past_the_range_fails_as_a_malformed_field_fails() {
    let (_past_dir, past) = repository(Value::from(1_i64 << 53));
    let (_malformed_dir, malformed) = repository(Value::from("soon"));
    for args in [
        &["check", "--format", "json"][..],
        &["resolution_a", "get", "--format", "json"][..],
    ] {
        let read = args[0] == "resolution_a";
        let (past_code, past_out, past_err) = run(&past, args);
        let (code, out, err) = run(&malformed, args);
        let past_text = String::from_utf8_lossy(&past_err).into_owned();
        assert_eq!(past_code, Some(1), "{args:?}: {past_text}");
        assert!(past_text.contains("res.jsonl"), "{args:?}: {past_text}");
        assert!(
            past_text.contains("9007199254740992"),
            "{args:?}: {past_text}"
        );
        assert_eq!(
            past_text.contains("invalid_state_file"),
            read,
            "{args:?}: {past_text}"
        );
        assert_eq!(
            (
                past_code,
                without_messages(&past_out),
                without_messages(&past_err)
            ),
            (code, without_messages(&out), without_messages(&err)),
            "{args:?}"
        );
    }
}
