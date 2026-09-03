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
            "statement": statement
        }],
        "rules": [
            {
                "key": "expiry",
                "requirement": "sharing",
                "statement": "Share links must expire within 30 days"
            },
            {
                "key": "revocation",
                "requirement": "sharing",
                "statement": "Share links must be revocable"
            }
        ]
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

fn resource<'a>(result: &'a Value, kind: &str, key: &str) -> &'a Value {
    result["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == kind && resource["key"] == key)
        .unwrap()
}

fn affected<'a>(result: &'a Value, rule_id: &str) -> &'a Value {
    result["affected_rules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|rule| rule["id"] == rule_id)
        .unwrap_or_else(|| panic!("rule {rule_id} is not reported as affected"))
}

const ORIGINAL: &str = "Users can securely share documentation";
const REVISED: &str = "Users can securely share documentation with named recipients";

struct Fixture {
    directory: tempfile::TempDir,
    requirement_id: String,
    expiry_id: String,
    revocation_id: String,
}

/// Applies the starting spec, implements one Rule, and verifies it.
fn fixture() -> Fixture {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let applied = sdk(repo, "apply", &spec(ORIGINAL));
    let requirement_id = resource(&applied, "requirement", "sharing")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let expiry_id = resource(&applied, "rule", "expiry")["id"]
        .as_str()
        .unwrap()
        .to_string();
    let revocation_id = resource(&applied, "rule", "revocation")["id"]
        .as_str()
        .unwrap()
        .to_string();
    std::fs::write(
        directory.path().join("share-links.ts"),
        format!("// @provenance rule: {expiry_id}\nexport function createShareLink() {{}}\n"),
    )
    .unwrap();
    verify(repo, &expiry_id);
    Fixture {
        directory,
        requirement_id,
        expiry_id,
        revocation_id,
    }
}

/// Records one passing verification run against a Rule.
fn verify(repo: &str, rule_id: &str) {
    let run = sdk(
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
    sdk(
        repo,
        "complete-verification",
        &json!({"run": run["id"], "status": "passed"}),
    );
}

#[test]
fn plan_explains_a_requirement_change_without_claiming_code_evidence_is_stale() {
    let fixture = fixture();
    let repo = fixture.directory.path().to_str().unwrap();

    let planned = sdk(repo, "plan", &spec(REVISED));

    let requirement = resource(&planned, "requirement", "sharing");
    assert_eq!(requirement["state"], "updated");
    assert_eq!(
        requirement["changes"],
        json!([{"field": "statement", "before": ORIGINAL, "after": REVISED}]),
        "plan must report the changed obligation with before and after"
    );

    let expiry = affected(&planned, &fixture.expiry_id);
    assert_eq!(
        expiry["implementations"][0],
        json!({"file": "share-links.ts", "line": 1}),
        "the implementation site must keep its existing shape"
    );
    assert_eq!(
        expiry["verifications"][0],
        json!({
            "key": "share-link-expiry",
            "method": "examples",
            "declared_by": "ci://typescript",
            "file": "share-links.test.ts",
            "symbol": "expiry test"
        }),
        "the verification site must keep its existing shape"
    );

    for rule_id in [&fixture.expiry_id, &fixture.revocation_id] {
        let evidence = &affected(&planned, rule_id)["evidence"];
        assert_eq!(
            evidence["review_required"], true,
            "a changed requirement statement puts {rule_id} evidence up for review"
        );
        assert_eq!(
            evidence["reasons"][0],
            json!({
                "requirement": fixture.requirement_id,
                "field": "statement",
                "before": ORIGINAL,
                "after": REVISED
            }),
            "the review reason must name the requirement and what changed"
        );
    }

    let rendered = serde_json::to_string(&planned).unwrap();
    assert!(
        !rendered.contains("stale"),
        "a requirement-only change must never claim code evidence is stale: {rendered}"
    );
}

#[test]
fn a_new_verification_run_clears_review_for_only_its_own_rule() {
    let fixture = fixture();
    let repo = fixture.directory.path().to_str().unwrap();
    sdk(repo, "apply", &spec(REVISED));

    let after_apply = sdk(repo, "plan", &spec(REVISED));
    for rule_id in [&fixture.expiry_id, &fixture.revocation_id] {
        assert_eq!(
            affected(&after_apply, rule_id)["evidence"]["review_required"],
            true,
            "the applied requirement change is persisted review knowledge for {rule_id}"
        );
    }

    verify(repo, &fixture.expiry_id);

    let after_rerun = sdk(repo, "plan", &spec(REVISED));
    assert_eq!(
        affected(&after_rerun, &fixture.expiry_id)["evidence"]["review_required"],
        false,
        "a verification run arriving after the change clears its own Rule"
    );
    assert_eq!(
        affected(&after_rerun, &fixture.revocation_id)["evidence"]["review_required"],
        true,
        "the Rule that was not rerun still needs review"
    );
}

#[test]
fn human_output_explains_the_change_in_plain_words() {
    let fixture = fixture();
    let repo = fixture.directory.path().to_str().unwrap();

    let output = provenance()
        .args([
            "sdk", "plan", "--repo", repo, "--scope", "default", "--format", "markdown",
        ])
        .write_stdin(serde_json::to_vec(&spec(REVISED)).unwrap())
        .output()
        .unwrap();
    let rendered = String::from_utf8(output.stdout).unwrap();

    assert!(
        rendered.contains(ORIGINAL) && rendered.contains(REVISED),
        "human output shows what the requirement said before and says now: {rendered}"
    );
    assert!(
        rendered.contains("review required"),
        "human output names the state in plain words: {rendered}"
    );
    assert!(
        rendered.contains("share-links.ts") && rendered.contains("share-links.test.ts"),
        "human output lists the sites that deserve attention: {rendered}"
    );
    assert!(
        !rendered.contains("stale"),
        "human output must not claim code evidence is stale: {rendered}"
    );
}
