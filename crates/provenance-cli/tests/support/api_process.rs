//! The `provenance api` command of one call and its answer, for the tests
//! that read the API through one process for each call.

use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Output};

/// The path without its leading `/`, and the query pairs of the path.
pub fn split(path: &str) -> (&str, Vec<(&str, &str)>) {
    let path = path.trim_start_matches('/');
    let Some((path, query)) = path.split_once('?') else {
        return (path, Vec::new());
    };
    let pairs = query
        .split('&')
        .map(|pair| pair.split_once('=').unwrap())
        .collect();
    (path, pairs)
}

/// The `provenance api` command of one call on the repository at `repo`.
/// With `input`, the command reads the request body on standard input.
pub fn command(
    binary: &Path,
    repo: &str,
    call: (&str, &str),
    headers: &[(&str, &str)],
    input: bool,
) -> Command {
    let (method, path) = call;
    let (path, query) = split(path);
    let mut command = Command::new(binary);
    command.args([
        "api", path, "--repo", repo, "--format", "json", "--method", method,
    ]);
    for (name, value) in query {
        command.args(["--query", &format!("{name}={value}")]);
    }
    for (name, value) in headers {
        command.args(["--header", &format!("{name}: {value}")]);
    }
    if input {
        command.args(["--input", "-"]);
    }
    command
}

/// The status and the body of the answer of one call: 200 and the output on
/// success, else 1 and both outputs.
pub fn answer(output: &Output) -> (u16, Value) {
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
    if output.status.success() {
        (200, serde_json::from_slice(&output.stdout).unwrap())
    } else {
        let stderr = text(&output.stderr);
        let stderr = serde_json::from_str(&stderr).unwrap_or(Value::String(stderr));
        (1, json!({"stdout": text(&output.stdout), "stderr": stderr}))
    }
}
