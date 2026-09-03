//! Golden pins for the human-rendered plan (V9).
//!
//! These pins land before the CLI SDK operations move to provenance-store
//! and must stay green after that move.

use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_macros::verifies;
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

fn spec(requirement: &str, statement: &str) -> Value {
    json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "share-links",
        "declared_by": "spec://typescript/share-links",
        "requirements": [{
            "key": "sharing",
            "statement": requirement
        }],
        "rules": [{
            "key": "expiry",
            "requirement": "sharing",
            "statement": statement
        }]
    })
}

fn sdk_json(repo: &str, command: &str, input: &Value) -> Value {
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

fn rendered_plan(repo: &str, input: &Value) -> String {
    let output = provenance()
        .args([
            "sdk", "plan", "--repo", repo, "--scope", "default", "--format", "markdown",
        ])
        .write_stdin(serde_json::to_vec(input).unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "sdk plan failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn rule_id(result: &Value) -> String {
    result["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == "rule")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn an_unchanged_plan_renders_the_empty_report() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let document = spec(
        "Users can securely share documentation",
        "Share links expire within 30 days",
    );
    sdk_json(repo, "apply", &document);

    assert_eq!(rendered_plan(repo, &document), "Nothing changes.\n");
}

#[test]
#[verifies("rule_rust_plan_goldens_precede_store_relocation", examples)]
fn a_rule_statement_change_renders_the_change_and_the_affected_rule() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let applied = sdk_json(
        repo,
        "apply",
        &spec(
            "Users can securely share documentation",
            "Share links expire within 30 days",
        ),
    );
    let rule = rule_id(&applied);

    let rendered = rendered_plan(
        repo,
        &spec(
            "Users can securely share documentation",
            "Share links expire within 14 days",
        ),
    );

    assert_eq!(
        rendered,
        format!(
            "What changed\n\
             \n\
             \x20 rule expiry ({rule})\n\
             \x20   statement was: Share links expire within 30 days\n\
             \x20   statement now: Share links expire within 14 days\n\
             \n\
             Rules that deserve attention: 1\n\
             \n\
             \x20 {rule}\n\
             \x20   no review outstanding\n\
             \x20   no implementation recorded\n\
             \x20   no verification recorded\n"
        )
    );
}

#[test]
fn a_requirement_restatement_renders_review_and_evidence_sites() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let applied = sdk_json(
        repo,
        "apply",
        &spec(
            "Users can securely share documentation",
            "Share links expire within 30 days",
        ),
    );
    let rule = rule_id(&applied);
    std::fs::write(
        directory.path().join("share-links.ts"),
        format!("// @provenance rule: {rule}\nexport function createShareLink() {{}}\n"),
    )
    .unwrap();
    sdk_json(
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

    let rendered = rendered_plan(
        repo,
        &spec(
            "Users can share documentation securely and privately",
            "Share links expire within 30 days",
        ),
    );

    assert_eq!(
        rendered,
        format!(
            "What changed\n\
             \n\
             \x20 requirement sharing (requirement_share-links_sharing_ff96f5fa62)\n\
             \x20   statement was: Users can securely share documentation\n\
             \x20   statement now: Users can share documentation securely and privately\n\
             \n\
             Rules that deserve attention: 1\n\
             \n\
             \x20 {rule}\n\
             \x20   review required, because the requirement it serves changed:\n\
             \x20     requirement_share-links_sharing_ff96f5fa62 statement changed from \"Users can securely share documentation\" to \"Users can share documentation securely and privately\"\n\
             \x20   a verification run recorded after that change clears this\n\
             \x20   implemented at:\n\
             \x20     share-links.ts:1\n\
             \x20   verified at:\n\
             \x20     share-links.test.ts (expiry test) [share-link-expiry, examples]\n"
        )
    );
}
