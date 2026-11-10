//! A typed authoring document that deletes a declaration changes the records
//! that cite it, or refuses before a write.
//!
//! These tests have no Rule binding. No Rule states the typed deletion
//! cascade yet; the tests keep the behaviour under a compiled target.
#[path = "cli_sdk_lifecycle/deletion.rs"]
mod deletion;

use assert_cmd::Command;
use serde_json::{json, Value};

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn init_repo() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    provenance()
        .args(["init", "--path", directory.path().to_str().unwrap()])
        .args(["--scope", "default", "--path-prefix", "."])
        .assert()
        .success();
    directory
}

/// Sends the document to the plan route or to the apply route and returns
/// the result data, or the error text.
fn sdk(repo: &str, command: &str, input: &Value) -> Result<Value, String> {
    let route = match command {
        "plan" => "authoring-plans",
        "apply" => "authoring-changes",
        other => panic!("unknown authoring command {other}"),
    };
    let output = provenance()
        .args(["api", route, "--method", "post", "--input", "-"])
        .args(["--repo", repo, "--scope", "default", "--format", "json"])
        .write_stdin(serde_json::to_vec(input).unwrap())
        .output()
        .unwrap();
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    Ok(response["data"].clone())
}

fn document(owner: &str) -> Value {
    json!({
        "spec": "share-links",
        "declared_by": owner,
        "sources": [{"key": "policy", "name": "Policy", "kind": "policy"}],
        "requirements": [{
            "key": "sharing", "statement": "The host stops.", "sources": ["policy"]
        }],
        "rules": [{"key": "expiry", "requirement": "sharing", "statement": "The host stops."}]
    })
}

fn empty_document(owner: &str) -> Value {
    json!({
        "spec": "share-links",
        "declared_by": owner,
        "sources": [],
        "requirements": [],
        "rules": []
    })
}

fn resource_id(result: &Value, kind: &str, key: &str) -> String {
    result["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == kind && resource["key"] == key)
        .unwrap_or_else(|| panic!("no {kind} {key} in {result}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn read_records(repo: &std::path::Path, path: &str) -> Vec<Value> {
    let path = repo.join(".provenance/state/scopes/default").join(path);
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
