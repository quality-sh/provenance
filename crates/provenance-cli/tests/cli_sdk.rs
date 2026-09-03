use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
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
        "sources": [{
            "key": "linear:ABC-123",
            "name": "Linear ABC-123",
            "kind": "linear",
            "url": "https://linear.app/example/issue/ABC-123"
        }],
        "requirements": [{
            "key": "sharing",
            "statement": "Users can securely share documentation",
            "sources": ["linear:ABC-123"]
        }],
        "rules": [{
            "key": "expiry",
            "requirement": "sharing",
            "statement": statement
        }]
    })
}

fn apply(repo: &str, input: &Value) -> Value {
    let output = provenance()
        .args([
            "sdk", "apply", "--repo", repo, "--scope", "default", "--format", "json",
        ])
        .write_stdin(serde_json::to_vec(input).unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "sdk apply failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn rule_id<'a>(result: &'a Value, parent: &str, key: &str) -> &'a str {
    result["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| {
            resource["kind"] == "rule" && resource["parent"] == parent && resource["key"] == key
        })
        .and_then(|resource| resource["id"].as_str())
        .unwrap()
}

#[test]
fn apply_materializes_typed_declarations_as_canonical_graph_records() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();

    let result = apply(repo, &spec("Share links expire within 30 days"));

    assert_eq!(result["created"], 3);
    assert_eq!(result["updated"], 0);
    assert_eq!(result["resources"][1]["key"], "sharing");
    assert!(result["resources"][1]["id"]
        .as_str()
        .unwrap()
        .starts_with("requirement_share-links_sharing_"));
    assert_eq!(result["resources"][2]["key"], "expiry");
    let expiry_id = rule_id(&result, "sharing", "expiry");
    assert!(expiry_id.starts_with("rule_share-links_sharing_expiry_"));
    assert!(result["resources"][0]["id"]
        .as_str()
        .unwrap()
        .starts_with("source_share-links_linear_abc-123_"));

    provenance()
        .args([
            "rules", "show", "--repo", repo, "--scope", "default", "--id", expiry_id, "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(contains("Share links expire within 30 days"))
        .stdout(contains("spec://typescript/share-links"));

    provenance()
        .args([
            "traceability",
            expiry_id,
            "--repo",
            repo,
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(contains("sharing"));

    let wiki = directory.path().join("wiki");
    provenance()
        .args([
            "wiki",
            "build",
            "--repo",
            repo,
            "--scope",
            "default",
            "--out",
            wiki.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();
    let rule_page =
        std::fs::read_to_string(wiki.join(format!("rules/{expiry_id}/index.html"))).unwrap();
    assert!(rule_page.contains("Share links expire within 30 days"));

    provenance()
        .args(["check", "--repo", repo, "--format", "json"])
        .assert()
        .success();
}

#[test]
fn apply_updates_only_records_owned_by_the_same_spec() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let initial = apply(repo, &spec("Share links expire within 30 days"));
    let expiry_id = rule_id(&initial, "sharing", "expiry").to_string();

    let result = apply(repo, &spec("Share links expire within 14 days"));

    assert_eq!(result["created"], 0);
    assert_eq!(result["updated"], 1);
    assert_eq!(result["unchanged"], 2);
    provenance()
        .args([
            "rules", "show", "--repo", repo, "--scope", "default", "--id", &expiry_id, "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(contains("Share links expire within 14 days"));
}

#[test]
fn apply_scopes_repeated_rule_keys_to_their_parent_requirements() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let input = json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "lifecycles",
        "declared_by": "spec://typescript/lifecycles",
        "requirements": [
            {
                "key": "sharing",
                "statement": "Users can securely share documentation"
            },
            {
                "key": "sessions",
                "statement": "User sessions are time bounded"
            }
        ],
        "rules": [
            {
                "key": "expiry",
                "requirement": "sharing",
                "statement": "Share links expire within 30 days"
            },
            {
                "key": "expiry",
                "requirement": "sessions",
                "statement": "Inactive sessions expire within 24 hours"
            }
        ]
    });

    let result = apply(repo, &input);
    let rules = result["resources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|resource| resource["kind"] == "rule")
        .collect::<Vec<_>>();

    assert_eq!(rules.len(), 2);
    assert_ne!(rules[0]["id"], rules[1]["id"]);
    provenance()
        .args(["check", "--repo", repo, "--format", "json"])
        .assert()
        .success();
}

#[test]
fn apply_persists_structured_declaration_addresses() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let input = json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "share-links",
        "declared_by": "spec://typescript",
        "requirements": [{
            "key": "sharing",
            "statement": "Users can securely share documentation"
        }],
        "rules": [{
            "key": "expiry",
            "requirement": "sharing",
            "statement": "Share links expire within 30 days"
        }]
    });

    let result = apply(repo, &input);
    let expiry_id = rule_id(&result, "sharing", "expiry");
    let requirement = result["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == "requirement")
        .unwrap();
    let rule = result["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == "rule")
        .unwrap();

    assert_eq!(
        requirement["address"],
        json!(["share-links", "requirement", "sharing"])
    );
    assert_eq!(
        rule["address"],
        json!(["share-links", "requirement", "sharing", "rule", "expiry"])
    );
    provenance()
        .args([
            "rules", "show", "--repo", repo, "--scope", "default", "--id", expiry_id, "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(contains("declaration_address"))
        .stdout(contains("share-links"));
}

#[test]
fn apply_refuses_to_take_over_an_unowned_record() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    provenance()
        .args([
            "requirements",
            "create",
            "--repo",
            repo,
            "--scope",
            "default",
            "--id",
            "sharing",
            "--statement",
            "Externally managed statement",
        ])
        .assert()
        .success();

    let mut desired = spec("Share links expire");
    desired["requirements"][0]["id"] = json!("sharing");
    provenance()
        .args(["sdk", "apply", "--repo", repo, "--scope", "default"])
        .write_stdin(serde_json::to_vec(&desired).unwrap())
        .assert()
        .failure()
        .stderr(contains("sharing").and(contains("not owned")));

    provenance()
        .args(["export", "--repo", repo, "--scope", "default"])
        .assert()
        .success()
        .stdout(contains("Externally managed statement"))
        .stdout(contains("Share links expire").not());
}

#[test]
fn verification_runs_are_linked_to_the_rule_and_record_the_outcome() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let applied = apply(repo, &spec("Share links expire within 30 days"));
    let expiry_id = rule_id(&applied, "sharing", "expiry");

    let begun = provenance()
        .args([
            "sdk",
            "begin-verification",
            "--repo",
            repo,
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .write_stdin(
            serde_json::to_vec(&json!({
                "rule": expiry_id,
                "key": "share-link-expiry",
                "method": "examples",
                "declared_by": "ci://node-test",
                "file": "share-links.test.ts",
                "symbol": "share links expire"
            }))
            .unwrap(),
        )
        .output()
        .unwrap();
    assert!(begun.status.success());
    let begun: Value = serde_json::from_slice(&begun.stdout).unwrap();
    let run = begun["id"].as_str().unwrap();
    let binding = begun["binding_id"].as_str().unwrap();

    provenance()
        .args([
            "sdk",
            "complete-verification",
            "--repo",
            repo,
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .write_stdin(
            serde_json::to_vec(&json!({
                "run": run,
                "status": "passed"
            }))
            .unwrap(),
        )
        .assert()
        .success()
        .stdout(contains("\"status\": \"passed\""));

    provenance()
        .args([
            "sdk",
            "verification-runs",
            "--repo",
            repo,
            "--scope",
            "default",
            "--rule",
            expiry_id,
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(contains("ci://node-test"))
        .stdout(contains(binding))
        .stdout(contains("share-links.test.ts"))
        .stdout(contains("\"status\": \"passed\""));
}

#[test]
fn verification_resolves_an_applied_rule_by_declaration_address() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let input = json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "share-links",
        "declared_by": "spec://typescript",
        "requirements": [{
            "key": "sharing",
            "statement": "Users can securely share documentation"
        }],
        "rules": [{
            "key": "expiry",
            "requirement": "sharing",
            "statement": "Share links expire within 30 days"
        }]
    });
    let applied = apply(repo, &input);
    let expiry_id = rule_id(&applied, "sharing", "expiry");

    provenance()
        .args([
            "sdk",
            "begin-verification",
            "--repo",
            repo,
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .write_stdin(
            serde_json::to_vec(&json!({
                "declaration": {
                    "declared_by": "spec://typescript",
                    "address": ["share-links", "requirement", "sharing", "rule", "expiry"]
                },
                "key": "share-link-expiry",
                "method": "examples",
                "declared_by": "ci://node-test",
                "file": "tests/share-links.test.ts"
            }))
            .unwrap(),
        )
        .assert()
        .success()
        .stdout(contains(expiry_id));
}

#[test]
fn verification_cannot_begin_for_an_unknown_rule() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();

    provenance()
        .args([
            "sdk",
            "begin-verification",
            "--repo",
            repo,
            "--scope",
            "default",
        ])
        .write_stdin(
            serde_json::to_vec(&json!({
                "rule": "missing",
                "key": "missing-rule-check",
                "method": "examples",
                "declared_by": "ci://node-test",
                "file": "tests/missing.test.ts"
            }))
            .unwrap(),
        )
        .assert()
        .failure()
        .stderr(contains("missing").and(contains("does not exist")));
}
