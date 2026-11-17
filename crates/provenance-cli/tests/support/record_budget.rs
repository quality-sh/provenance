//! A repository and the CLI calls that the tests of the record budget share.

use serde_json::{json, Value};
use std::path::Path;
use std::process::Output;

use super::review_paths::provenance;

/// The bytes of one stored row of a record or a binding. The store owns the
/// number in `read_budget::RECORD_BYTES`.
pub const BUDGET: usize = 65_536;

/// A repository with the Requirement `req_data`, the Rule `rule_one` that
/// refines it, and the file `check.rs`.
pub fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    let root = repo.to_str().unwrap();
    provenance()
        .args(["init", "--path", root, "--scope", "default"])
        .args(["--path-prefix", "."])
        .current_dir(repo)
        .assert()
        .success();
    success(&cli(
        repo,
        &["req_data", "create", "--type", "requirement"],
        &[("--statement", "The system keeps the data")],
    ));
    success(&create_rule(repo, "rule_one", "The rule of the data"));
    std::fs::write(repo.join("check.rs"), "fn check() {}\n").unwrap();
    directory
}

/// Runs the CLI on `repo` in the scope `default` with `args`, then each
/// option of `options` with its value.
pub fn cli(repo: &Path, args: &[&str], options: &[(&str, &str)]) -> Output {
    let mut command = provenance();
    command
        .args(args)
        .args(["--repo", repo.to_str().unwrap(), "--scope", "default"])
        .current_dir(repo);
    for (option, value) in options {
        command.args([option, value]);
    }
    command.output().unwrap()
}

/// Runs the `api` command on `path`, with `input` as the body of a post.
pub fn api(repo: &Path, path: &str, input: Option<&Value>) -> Output {
    let mut command = provenance();
    command
        .args(["api", path, "--repo", repo.to_str().unwrap()])
        .args(["--scope", "default"])
        .current_dir(repo);
    if let Some(input) = input {
        command
            .args(["--method", "post", "--input", "-"])
            .write_stdin(input.to_string());
    }
    command.output().unwrap()
}

/// Runs the write `args` on `repo` in the scope `default` with `fields` on
/// standard input. A long value goes on standard input, because Windows
/// limits a command line to 32 767 characters.
pub fn write(repo: &Path, args: &[&str], fields: &Value) -> Output {
    let mut command = provenance();
    command
        .args(args)
        .args([
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--stdin",
        ])
        .current_dir(repo)
        .write_stdin(fields.to_string());
    command.output().unwrap()
}

/// Creates the Rule `id` of `req_data` with `description`.
pub fn create_rule(repo: &Path, id: &str, description: &str) -> Output {
    write(
        repo,
        &[id, "create", "--type", "rule"],
        &json!({
            "statement": "The system shows the data",
            "requirement_ids": ["req_data"],
            "severity": "high",
            "description": description
        }),
    )
}

/// The JSON that a command that passed printed.
pub fn success(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or(Value::Null)
}

/// The error of a command that was refused: exit code 1, nothing on standard
/// output, and the refusal on standard error.
pub fn refusal(output: &Output) -> Value {
    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    let stderr = String::from_utf8(output.stderr.clone()).unwrap();
    let message = stderr.trim_end().strip_prefix("Error: ").unwrap();
    serde_json::from_str::<Value>(message).unwrap()["error"].clone()
}

/// The refusal of a row of `bytes` bytes of the record `record_id`.
pub fn too_large(record_id: &str, bytes: usize) -> Value {
    json!({
        "kind": "record_too_large",
        "record_id": record_id,
        "bytes": bytes,
        "limit": BUDGET
    })
}
