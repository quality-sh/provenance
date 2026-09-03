#![allow(dead_code)]

use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use serde_json::{json, Value};

pub fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

pub fn init_repo() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    provenance()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    directory
}

pub fn sdk(repo: &str, command: &str, input: &Value) -> Value {
    let output = provenance()
        .args([
            "sdk", command, "--repo", repo, "--scope", "default", "--format", "json",
        ])
        .write_stdin(serde_json::to_vec(input).unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "sdk {command} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

pub fn sdk_raw(repo: &str, command: &str, input: &Value) -> (bool, String, String) {
    let output = provenance()
        .args([
            "sdk", command, "--repo", repo, "--scope", "default", "--format", "json",
        ])
        .write_stdin(serde_json::to_vec(input).unwrap())
        .output()
        .unwrap();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

pub fn sdk_error(repo: &str, command: &str, input: &Value) -> String {
    let (success, stdout, stderr) = sdk_raw(repo, command, input);
    assert!(!success, "sdk {command} unexpectedly succeeded: {stdout}");
    stderr
}

/// Two Requirements sharing one Rule, both grounded in one Source.
pub fn shared_rule_spec() -> Value {
    json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "share-links",
        "declared_by": "spec://typescript/share-links",
        "sources": [{
            "key": "retention",
            "name": "Retention policy",
            "kind": "policy",
            "reference": "docs/retention.md"
        }],
        "requirements": [
            {
                "key": "sharing",
                "statement": "Shares are time bounded",
                "sources": ["retention"]
            },
            {
                "key": "sessions",
                "statement": "Sessions are time bounded",
                "sources": ["retention"]
            }
        ],
        "rules": [{
            "key": "expiry",
            "requirements": ["sharing", "sessions"],
            "statement": "Share links expire within 30 days",
            "implementation": {
                "file": "share-links.ts",
                "symbol": "createShareLink"
            }
        }]
    })
}

/// Applies the shared-Rule spec and returns the canonical IDs it minted.
pub fn apply_shared_rule(directory: &tempfile::TempDir) -> Ids {
    let repo = directory.path().to_str().unwrap();
    std::fs::write(
        directory.path().join("share-links.ts"),
        "export function createShareLink() {}\n",
    )
    .unwrap();
    let result = sdk(repo, "apply", &shared_rule_spec());
    Ids {
        source: resource_id(&result, "source", "retention"),
        sharing: resource_id(&result, "requirement", "sharing"),
        sessions: resource_id(&result, "requirement", "sessions"),
        rule: resource_id(&result, "rule", "expiry"),
    }
}

/// Records one passing verification run against a Rule.
pub fn verify_rule(repo: &str, rule: &str) -> Value {
    let run = sdk(
        repo,
        "begin-verification",
        &json!({
            "rule": rule,
            "key": "share-link-expiry",
            "method": "examples",
            "declared_by": "ci://typescript",
            "file": "share-links.test.ts",
            "symbol": "expiry test"
        }),
    );
    sdk(
        repo,
        "complete-verification",
        &json!({"run": run["id"].as_str().unwrap(), "status": "passed"}),
    )
}

/// Restates the shared Requirement so its Rule's evidence needs review.
pub fn restate_sharing(repo: &str) -> Value {
    let mut restated = shared_rule_spec();
    restated["requirements"][0]["statement"] = json!("Shares are time bounded and revocable");
    sdk(repo, "apply", &restated)
}

pub struct Ids {
    pub source: String,
    pub sharing: String,
    pub sessions: String,
    pub rule: String,
}

pub fn resource_id(result: &Value, kind: &str, key: &str) -> String {
    result["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == kind && resource["key"] == key)
        .unwrap_or_else(|| panic!("no {kind} resource keyed {key} in {result}"))["id"]
        .as_str()
        .unwrap()
        .to_string()
}

pub fn ids_of(nodes: &Value) -> Vec<String> {
    nodes
        .as_array()
        .unwrap()
        .iter()
        .map(|node| node["id"].as_str().unwrap().to_string())
        .collect()
}
