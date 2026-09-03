use assert_cmd::Command;
use predicates::str::contains;
use serde_json::{json, Value};
use std::fs;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
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
    directory
}

fn apply(repo: &str, input: &Value) -> Result<Value, String> {
    let output = provenance()
        .args([
            "sdk", "apply", "--repo", repo, "--scope", "default", "--format", "json",
        ])
        .write_stdin(serde_json::to_vec(input).unwrap())
        .output()
        .unwrap();
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(serde_json::from_slice(&output.stdout).unwrap())
}

fn requirements() -> Value {
    json!([
        { "key": "sharing", "statement": "Shares are time bounded" },
        { "key": "sessions", "statement": "Sessions are time bounded" }
    ])
}

fn document(rules: Value) -> Value {
    let mut document = json!({
        "schema_version": 1,
        "spec": "lifecycles",
        "declared_by": "spec://typescript/lifecycles",
        "requirements": requirements(),
        "rules": []
    });
    document["rules"] = rules;
    document
}

fn rule_resource(result: &Value) -> &Value {
    result["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == "rule")
        .unwrap()
}

fn resource_id<'a>(result: &'a Value, kind: &str, key: &str) -> &'a str {
    result["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == kind && resource["key"] == key)
        .unwrap()["id"]
        .as_str()
        .unwrap()
}

#[test]
fn shared_rule_materializes_once_with_two_relationships() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let result = apply(
        repo,
        &document(json!([{
            "key": "expiry",
            "requirements": ["sharing", "sessions"],
            "statement": "Authenticated access expires"
        }])),
    )
    .unwrap();
    let resource = rule_resource(&result);
    let sharing_id = resource_id(&result, "requirement", "sharing");
    let sessions_id = resource_id(&result, "requirement", "sessions");

    assert_eq!(resource["address"], json!(["lifecycles", "rule", "expiry"]));
    assert!(resource.get("parent").is_none());
    let graph = provenance()
        .args([
            "graph", sharing_id, "--repo", repo, "--scope", "default", "--format", "json",
        ])
        .output()
        .unwrap();
    assert!(graph.status.success());
    provenance()
        .args([
            "graph",
            sessions_id,
            "--repo",
            repo,
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(contains(resource["id"].as_str().unwrap()));
}

#[test]
fn an_explicit_spec_scoped_address_stays_shared_with_one_requirement() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let result = apply(
        repo,
        &document(json!([{
            "key": "expiry",
            "address": ["lifecycles", "rule", "expiry"],
            "requirements": ["sharing"],
            "statement": "Share links expire"
        }])),
    )
    .unwrap();

    assert_eq!(
        rule_resource(&result)["address"],
        json!(["lifecycles", "rule", "expiry"])
    );
    assert!(rule_resource(&result).get("parent").is_none());
}

#[test]
fn an_explicit_local_address_must_name_its_only_requirement() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let error = apply(
        repo,
        &document(json!([{
            "key": "expiry",
            "address": ["lifecycles", "requirement", "sessions", "rule", "expiry"],
            "requirements": ["sharing"],
            "statement": "Share links expire"
        }])),
    )
    .unwrap_err();

    assert!(error.contains("address"), "{error}");
    assert!(error.contains("sharing"), "{error}");
}

#[test]
fn inferred_local_to_explicit_shared_with_one_requirement_preserves_id() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let local = document(json!([{
        "key": "expiry",
        "requirements": ["sharing"],
        "statement": "Share links expire"
    }]));
    let shared = document(json!([{
        "key": "expiry",
        "address": ["lifecycles", "rule", "expiry"],
        "requirements": ["sharing"],
        "statement": "Share links expire"
    }]));

    let first = apply(repo, &local).unwrap();
    let second = apply(repo, &shared).unwrap();

    assert_eq!(rule_resource(&second)["id"], rule_resource(&first)["id"]);
    assert_eq!(
        rule_resource(&second)["address"],
        json!(["lifecycles", "rule", "expiry"])
    );
}

#[test]
fn explicit_address_rejects_the_wrong_spec() {
    let directory = init_repo();
    let error = apply(
        directory.path().to_str().unwrap(),
        &document(json!([{
            "key": "expiry",
            "address": ["another-spec", "rule", "expiry"],
            "requirements": ["sharing"],
            "statement": "Share links expire"
        }])),
    )
    .unwrap_err();

    assert!(error.contains("address"), "{error}");
    assert!(error.contains("lifecycles"), "{error}");
}

#[test]
fn explicit_address_rejects_the_wrong_key_and_arbitrary_shape() {
    for address in [
        json!(["lifecycles", "rule", "another-key"]),
        json!(["lifecycles", "rules", "expiry"]),
        json!(["lifecycles", "rule", "expiry", "extra"]),
    ] {
        let directory = init_repo();
        let error = apply(
            directory.path().to_str().unwrap(),
            &document(json!([{
                "key": "expiry",
                "address": address,
                "requirements": ["sharing"],
                "statement": "Share links expire"
            }])),
        )
        .unwrap_err();

        assert!(error.contains("address"), "{error}");
        assert!(error.contains("expiry"), "{error}");
    }
}

#[test]
fn explicit_local_address_rejects_several_requirements() {
    let directory = init_repo();
    let error = apply(
        directory.path().to_str().unwrap(),
        &document(json!([{
            "key": "expiry",
            "address": ["lifecycles", "requirement", "sharing", "rule", "expiry"],
            "requirements": ["sharing", "sessions"],
            "statement": "Authenticated access expires"
        }])),
    )
    .unwrap_err();

    assert!(error.contains("shared address"), "{error}");
    assert!(error.contains("lifecycles/rule/expiry"), "{error}");
}

#[test]
fn local_to_shared_and_shared_to_local_preserve_the_canonical_id() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let local = document(json!([{
        "key": "expiry",
        "requirements": ["sharing"],
        "statement": "Authenticated access expires"
    }]));
    let shared = document(json!([{
        "key": "expiry",
        "requirements": ["sharing", "sessions"],
        "statement": "Authenticated access expires"
    }]));

    let first = apply(repo, &local).unwrap();
    let first_id = rule_resource(&first)["id"].clone();
    let second = apply(repo, &shared).unwrap();
    let third = apply(repo, &local).unwrap();

    assert_eq!(rule_resource(&second)["id"], first_id);
    assert_eq!(
        rule_resource(&second)["address"],
        json!(["lifecycles", "rule", "expiry"])
    );
    assert_eq!(rule_resource(&third)["id"], first_id);
    assert_eq!(
        rule_resource(&third)["address"],
        json!(["lifecycles", "requirement", "sharing", "rule", "expiry"])
    );
}

#[test]
fn merging_multiple_local_candidates_requires_an_explicit_existing_id() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let locals = apply(
        repo,
        &document(json!([
            {
                "key": "expiry",
                "requirements": ["sharing"],
                "statement": "Share links expire"
            },
            {
                "key": "expiry",
                "requirements": ["sessions"],
                "statement": "Sessions expire"
            }
        ])),
    )
    .unwrap();
    let sharing_id = locals["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == "rule" && resource["parent"] == "sharing")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let sessions_id = locals["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == "rule" && resource["parent"] == "sessions")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let error = apply(
        repo,
        &document(json!([{
            "key": "expiry",
            "requirements": ["sharing", "sessions"],
            "statement": "Authenticated access expires"
        }])),
    )
    .unwrap_err();

    assert!(error.contains("ambiguous"), "{error}");
    assert!(error.contains("explicit"), "{error}");
    assert!(error.contains("expiry"), "{error}");

    let resolved = apply(
        repo,
        &document(json!([{
            "key": "expiry",
            "id": sharing_id,
            "requirements": ["sharing", "sessions"],
            "statement": "Authenticated access expires"
        }])),
    )
    .unwrap();
    assert_eq!(rule_resource(&resolved)["id"], sharing_id);
    assert_eq!(
        rule_resource(&resolved)["address"],
        json!(["lifecycles", "rule", "expiry"])
    );
    let records = read_jsonl(
        &directory
            .path()
            .join(".provenance/state/scopes/default/rules/rule.jsonl"),
    );
    assert_eq!(records.len(), 2);
    assert!(records
        .iter()
        .any(|record| record["id"] == sharing_id && record.get("retired").is_none()));
    assert!(records
        .iter()
        .any(|record| record["id"] == sessions_id && record["retired"] == true));
    assert_eq!(
        records
            .iter()
            .map(|record| record["requirement_ids"].as_array().map_or(0, Vec::len))
            .sum::<usize>(),
        3
    );
}

#[test]
fn legacy_single_requirement_documents_remain_accepted() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let result = apply(
        repo,
        &document(json!([{
            "key": "expiry",
            "requirement": "sharing",
            "statement": "Share links expire"
        }])),
    )
    .unwrap();

    assert_eq!(
        rule_resource(&result)["address"],
        json!(["lifecycles", "requirement", "sharing", "rule", "expiry"])
    );
}

fn read_jsonl(path: &std::path::Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
