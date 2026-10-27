// Each test binary that includes this file uses only some of the helpers.
#![allow(dead_code)]

use assert_cmd::Command;
use serde_json::Value;

pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);

pub fn provenance() -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("provenance"));
    // Implementation aid: report a stalled child instead of blocking the suite.
    command.timeout(std::time::Duration::from_secs(60));
    command
}

pub fn init() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    init_scope(directory.path(), "default", ".");
    (directory, repo)
}

/// Declares one scope in the repository. The first call creates the graph
/// and names the reviewer.
pub fn init_scope(repo: &std::path::Path, scope: &str, prefix: &str) {
    let mut args = vec![
        "init",
        "--path",
        repo.to_str().unwrap(),
        "--scope",
        scope,
        "--path-prefix",
        prefix,
    ];
    if !repo.join(".provenance").exists() {
        args.extend(["--disposition-actor-id", "reviewer"]);
    }
    provenance().args(args).assert().success();
}

pub fn json_output(command: &mut Command) -> Value {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

pub fn create_requirement(repo: &str, id: &str) -> Value {
    json_output(provenance().args([
        "requirements",
        "create",
        "--repo",
        repo,
        "--id",
        id,
        "--statement",
        "The system keeps review feedback available.",
        "--format",
        "json",
    ]))
}
