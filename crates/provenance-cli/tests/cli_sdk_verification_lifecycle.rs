use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use serde_json::{json, Value};
use std::fs;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn document() -> Value {
    json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "workflow-runtime",
        "declared_by": "spec://typescript/workflow-runtime",
        "requirements": [{
            "key": "workflows",
            "statement": "Accepted workflows execute"
        }],
        "rules": [
            {
                "key": "start",
                "requirement": "workflows",
                "statement": "Accepted workflows start"
            },
            {
                "key": "resume",
                "requirement": "workflows",
                "statement": "Suspended workflows resume"
            }
        ]
    })
}

fn init_repo() -> tempfile::TempDir {
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
    fs::create_dir(directory.path().join("tests")).unwrap();
    fs::write(directory.path().join("tests/expiry.test.ts"), "\n").unwrap();
    fs::write(directory.path().join("tests/other.test.ts"), "\n").unwrap();
    sdk(directory.path(), "apply", &document()).unwrap();
    directory
}

fn sdk(repo: &std::path::Path, command: &str, input: &Value) -> Result<Value, String> {
    let output = provenance()
        .args([
            "sdk",
            command,
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .write_stdin(serde_json::to_vec(input).unwrap())
        .output()
        .unwrap();
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(serde_json::from_slice(&output.stdout).unwrap())
}

fn rule_id(repo: &std::path::Path, key: &str) -> String {
    let planned = sdk(repo, "plan", &document()).unwrap();
    planned["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == "rule" && resource["key"] == key)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[derive(Clone, Copy)]
struct Verification<'a> {
    rule: &'a str,
    key: &'a str,
    owner: &'a str,
    file: &'a str,
}

fn verify(repo: &std::path::Path, verification: Verification<'_>) -> String {
    sdk(
        repo,
        "begin-verification",
        &json!({
            "rule": verification.rule,
            "key": verification.key,
            "method": "examples",
            "declared_by": verification.owner,
            "file": verification.file
        }),
    )
    .unwrap()["binding_id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn stored_bindings(repo: &std::path::Path) -> Vec<Value> {
    fs::read_to_string(repo.join(".provenance/state/scopes/default/verifications/binding.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn binding<'a>(stored: &'a [Value], id: &str) -> &'a Value {
    stored.iter().find(|record| record["id"] == id).unwrap()
}

#[test]
fn the_same_key_reported_from_another_file_leaves_the_first_file_alone() {
    let directory = init_repo();
    let start = rule_id(directory.path(), "start");
    let resume = rule_id(directory.path(), "resume");
    verify(
        directory.path(),
        Verification {
            rule: &start,
            key: "expiry",
            owner: "ci://typescript",
            file: "tests/expiry.test.ts",
        },
    );

    verify(
        directory.path(),
        Verification {
            rule: &resume,
            key: "expiry",
            owner: "ci://typescript",
            file: "tests/other.test.ts",
        },
    );

    let stored = stored_bindings(directory.path());
    assert_eq!(stored.len(), 2);
}

fn unverified_rules(repo: &std::path::Path) -> String {
    let output = provenance()
        .args([
            "coverage",
            "scan",
            "--repo",
            repo.to_str().unwrap(),
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--validate-rules",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

fn exported_bindings(repo: &std::path::Path) -> Vec<Value> {
    let output = provenance()
        .args([
            "export",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["verification_bindings"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn replacing_a_key_deletes_only_its_previous_binding() {
    let directory = init_repo();
    let repo = directory.path();
    let start = rule_id(repo, "start");
    let resume = rule_id(repo, "resume");
    let original = Verification {
        rule: &start,
        key: "expiry",
        owner: "ci://typescript",
        file: "tests/expiry.test.ts",
    };
    let replaced = verify(repo, original);
    let preserved = verify(
        repo,
        Verification {
            owner: "ci://other",
            ..original
        },
    );
    let replacement = verify(
        repo,
        Verification {
            rule: &resume,
            ..original
        },
    );
    let stored = stored_bindings(repo);
    assert_eq!(stored.len(), 2);
    assert!(!stored.iter().any(|r| r["id"] == replaced));
    assert_eq!(binding(&stored, &preserved)["rule_id"], start);
    assert_eq!(binding(&stored, &replacement)["rule_id"], resume);
    assert_eq!(exported_bindings(repo), stored);
    let empty_run = document();
    sdk(repo, "apply", &empty_run).unwrap();
    assert_eq!(stored_bindings(repo), stored);
}

#[test]
fn deleting_a_rule_makes_its_remaining_marker_an_unknown_id_warning() {
    let directory = init_repo();
    let repo = directory.path();
    let id = rule_id(repo, "start");
    fs::write(
        repo.join("marker.rs"),
        format!("#[rule(\"{id}\")]\nfn run() {{}}\n"),
    )
    .unwrap();
    let mut input = document();
    input["rules"].as_array_mut().unwrap().remove(0);
    sdk(repo, "apply", &input).unwrap();
    let scan: Value = serde_json::from_str(&unverified_rules(repo)).unwrap();
    assert!(scan["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w["rule_id"] == id && w["message"].as_str().unwrap().contains("unknown")));
}
