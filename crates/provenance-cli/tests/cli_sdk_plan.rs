use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
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

fn spec(statement: &str) -> Value {
    json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "share-links",
        "declared_by": "spec://typescript/share-links",
        "requirements": [{
            "key": "sharing",
            "statement": "Users can securely share documentation"
        }],
        "rules": [{
            "key": "expiry",
            "requirement": "sharing",
            "statement": statement
        }]
    })
}

fn sdk(repo: &str, command: &str, input: &Value) -> Value {
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

fn rule(result: &Value) -> &Value {
    result["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == "rule")
        .unwrap()
}

#[test]
fn plan_previews_semantic_changes_and_affected_rule_sites_without_writing() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let initial = sdk(repo, "apply", &spec("Share links expire within 30 days"));
    let rule_id = rule(&initial)["id"].as_str().unwrap();
    std::fs::write(
        directory.path().join("share-links.ts"),
        format!("// @provenance rule: {rule_id}\nexport function createShareLink() {{}}\n"),
    )
    .unwrap();
    sdk(
        repo,
        "begin-verification",
        &json!({
            "rule": rule_id,
            "key": "share-link-expiry",
            "method": "examples",
            "declared_by": "ci://typescript",
            "file": "share-links.test.ts",
            "symbol": "expiry test"
        }),
    );

    let planned = sdk(repo, "plan", &spec("Share links expire within 14 days"));

    assert_eq!(planned["updated"], 1);
    assert_eq!(
        rule(&planned)["changes"],
        json!([{
            "field": "statement",
            "before": "Share links expire within 30 days",
            "after": "Share links expire within 14 days"
        }])
    );
    assert_eq!(planned["affected_rules"][0]["id"], rule_id);
    assert_eq!(
        planned["affected_rules"][0]["implementations"][0],
        json!({"file": "share-links.ts", "line": 1})
    );
    assert_eq!(
        planned["affected_rules"][0]["verifications"][0],
        json!({
            "key": "share-link-expiry",
            "method": "examples",
            "declared_by": "ci://typescript",
            "file": "share-links.test.ts",
            "symbol": "expiry test"
        })
    );

    let unchanged = sdk(repo, "plan", &spec("Share links expire within 30 days"));
    assert_eq!(unchanged["updated"], 0, "plan must not mutate the Rule");
    assert!(unchanged["affected_rules"].as_array().unwrap().is_empty());

    let mut requirement_change = spec("Share links expire within 30 days");
    requirement_change["requirements"][0]["statement"] =
        json!("Users can share documentation securely and privately");
    let requirement_plan = sdk(repo, "plan", &requirement_change);
    assert_eq!(requirement_plan["affected_rules"][0]["id"], rule_id);

    let applied = sdk(repo, "apply", &spec("Share links expire within 14 days"));
    assert_eq!(rule(&applied), rule(&planned));
    assert_eq!(
        sdk(repo, "plan", &spec("Share links expire within 14 days"))["updated"],
        0
    );
}

#[test]
fn plan_previews_creates_without_materializing_them() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();

    let first = sdk(repo, "plan", &spec("Share links expire within 30 days"));
    let second = sdk(repo, "plan", &spec("Share links expire within 30 days"));

    assert_eq!(first["created"], 2);
    assert_eq!(second["created"], 2);
    assert_eq!(first, second);
}
