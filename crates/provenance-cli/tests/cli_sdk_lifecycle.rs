use assert_cmd::Command;
use serde_json::{json, Value};

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

fn sdk(repo: &str, command: &str, input: &Value) -> Result<Value, String> {
    let output = provenance()
        .args([
            "sdk", command, "--repo", repo, "--scope", "default", "--format", "json",
        ])
        .write_stdin(serde_json::to_vec(input).unwrap())
        .output()
        .unwrap();
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(serde_json::from_slice(&output.stdout).unwrap())
}

fn document(owner: &str) -> Value {
    json!({
        "schema_version": 1,
        "spec": "share-links",
        "declared_by": owner,
        "sources": [{
            "key": "policy",
            "name": "Sharing policy",
            "kind": "policy"
        }],
        "requirements": [{
            "key": "sharing",
            "statement": "Users can securely share documentation",
            "sources": ["policy"]
        }],
        "rules": [{
            "key": "expiry",
            "requirement": "sharing",
            "statement": "Share links expire within 30 days"
        }]
    })
}

fn empty_document(owner: &str) -> Value {
    json!({
        "schema_version": 1,
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
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn omission_plans_and_applies_retirement_without_deleting_history() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let owner = "spec://typescript/share-links";
    let initial = sdk(repo, "apply", &document(owner)).unwrap();
    let ids = [
        resource_id(&initial, "source", "policy"),
        resource_id(&initial, "requirement", "sharing"),
        resource_id(&initial, "rule", "expiry"),
    ];

    let planned = sdk(repo, "plan", &empty_document(owner)).unwrap();
    assert_eq!(planned["retired"], 3);
    assert!(planned["resources"]
        .as_array()
        .unwrap()
        .iter()
        .all(|resource| resource["state"] == "retired"));
    assert!(read_records(directory.path(), "sources/source.jsonl")
        .iter()
        .all(|record| record.get("retired").is_none()));

    let applied = sdk(repo, "apply", &empty_document(owner)).unwrap();
    assert_eq!(applied["retired"], planned["retired"]);
    assert_eq!(applied["resources"], planned["resources"]);
    for relative in [
        "sources/source.jsonl",
        "requirements/req.jsonl",
        "rules/rule.jsonl",
    ] {
        assert_eq!(read_records(directory.path(), relative)[0]["retired"], true);
    }
    let verification_error = sdk(
        repo,
        "begin-verification",
        &json!({
            "declaration": {
                "declared_by": owner,
                "address": ["share-links", "requirement", "sharing", "rule", "expiry"]
            },
            "key": "expiry-after-retirement",
            "method": "examples",
            "declared_by": "ci://typescript",
            "file": "tests/share-links.test.ts"
        }),
    )
    .unwrap_err();
    assert!(
        verification_error.contains("retired"),
        "{verification_error}"
    );

    let clean = sdk(repo, "plan", &empty_document(owner)).unwrap();
    assert_eq!(clean["retired"], 0);
    assert!(clean["resources"].as_array().unwrap().is_empty());

    let reactivated = sdk(repo, "apply", &document(owner)).unwrap();
    assert_eq!(reactivated["updated"], 3);
    assert_eq!(resource_id(&reactivated, "source", "policy"), ids[0]);
    assert_eq!(resource_id(&reactivated, "requirement", "sharing"), ids[1]);
    assert_eq!(resource_id(&reactivated, "rule", "expiry"), ids[2]);
}

#[test]
fn moving_a_rule_between_requirements_preserves_id_and_reparents_the_edge() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let owner = "spec://typescript/lifecycles";
    let mut initial = document(owner);
    initial["spec"] = json!("lifecycles");
    initial["requirements"].as_array_mut().unwrap().push(json!({
        "key": "sessions",
        "statement": "Sessions expire"
    }));
    let first = sdk(repo, "apply", &initial).unwrap();
    let rule_id = resource_id(&first, "rule", "expiry");
    let sessions_id = resource_id(&first, "requirement", "sessions");

    let mut moved = initial.clone();
    moved["rules"][0]["id"] = json!(rule_id);
    moved["rules"][0]["requirement"] = json!("sessions");
    let planned = sdk(repo, "plan", &moved).unwrap();
    let rule = planned["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == "rule")
        .unwrap();
    assert_eq!(planned["moved"], 1);
    assert_eq!(rule["state"], "moved");
    assert_eq!(rule["id"], rule_id);
    assert_eq!(rule["parent"], "sessions");

    sdk(repo, "apply", &moved).unwrap();
    let rules = read_records(directory.path(), "rules/rule.jsonl");
    let moved = rules.iter().find(|rule| rule["id"] == rule_id).unwrap();
    assert_eq!(moved["requirement_ids"], json!([sessions_id]));
}

#[test]
fn plan_returns_an_ownership_conflict_while_apply_refuses_it() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let first = sdk(repo, "apply", &document("spec://typescript/first")).unwrap();
    let source_id = resource_id(&first, "source", "policy");
    let conflicting = json!({
        "schema_version": 1,
        "spec": "other",
        "declared_by": "spec://typescript/other",
        "sources": [{
            "key": "policy",
            "id": source_id,
            "name": "Other policy",
            "kind": "policy"
        }]
    });

    let planned = sdk(repo, "plan", &conflicting).unwrap();
    assert_eq!(planned["conflicts"], 1);
    assert_eq!(planned["resources"][0]["state"], "conflict");
    assert_eq!(
        planned["resources"][0]["changes"][0]["field"],
        "declared_by"
    );

    let error = sdk(repo, "apply", &conflicting).unwrap_err();
    assert!(error.contains("not owned"), "{error}");
}

fn read_records(root: &std::path::Path, relative: &str) -> Vec<Value> {
    std::fs::read_to_string(root.join(".provenance/state/scopes/default").join(relative))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
