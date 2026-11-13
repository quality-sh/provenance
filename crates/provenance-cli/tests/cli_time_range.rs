//! A time field past the range of the read database, ±(2^53 − 1), in a
//! committed record fails as a malformed field fails: `check` and a read
//! refuse with `invalid_state_file` at the line of the record.
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

/// The text of `output` with the value of each `"message":"..."` left out.
fn without_messages(output: &[u8]) -> String {
    let text = String::from_utf8_lossy(output);
    let mut kept = String::new();
    let mut rest = text.as_ref();
    while let Some(at) = rest.find("\"message\":\"") {
        let start = at + "\"message\":\"".len();
        kept.push_str(&rest[..start]);
        let after = &rest[start..];
        let mut end = 0;
        let mut escaped = false;
        for (index, c) in after.char_indices() {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                end = index;
                break;
            }
        }
        rest = &after[end..];
    }
    kept.push_str(rest);
    kept
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
        let (past_code, past_out, past_err) = run(&past, args);
        let (code, out, err) = run(&malformed, args);
        let past_text = String::from_utf8_lossy(&past_err).into_owned();
        assert_eq!(past_code, Some(1), "{args:?}: {past_text}");
        assert!(
            past_text.contains("invalid_state_file"),
            "{args:?}: {past_text}"
        );
        assert!(
            past_text.contains("9007199254740992"),
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
