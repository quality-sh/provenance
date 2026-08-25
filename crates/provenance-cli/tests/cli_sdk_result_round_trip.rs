//! The result-side round-trip guard (DR6): decoding CLI-emitted JSON
//! into the store's result and plan types, then encoding again, keeps
//! every byte and every omission.

use assert_cmd::Command;
use provenance_store::operations::TypedSpecPlan;
use provenance_store::state_store::TypedSpecResult;
use serde_json::json;

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

fn emitted(repo: &str, command: &str) -> String {
    let input = json!({
        "schema_version": 1,
        "spec": "share-links",
        "declared_by": "spec://typescript/share-links",
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
    let output = provenance()
        .args([
            "sdk", command, "--repo", repo, "--scope", "default", "--format", "json",
        ])
        .write_stdin(serde_json::to_vec(&input).unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "sdk {command} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn an_apply_result_decodes_and_encodes_to_the_same_bytes() {
    let directory = init_repo();
    let emitted = emitted(directory.path().to_str().unwrap(), "apply");

    let decoded: TypedSpecResult = serde_json::from_str(&emitted).unwrap();

    assert_eq!(
        serde_json::to_string_pretty(&decoded).unwrap(),
        emitted.trim_end()
    );
}

#[test]
fn a_plan_decodes_and_encodes_to_the_same_bytes() {
    let directory = init_repo();
    let emitted = emitted(directory.path().to_str().unwrap(), "plan");

    let decoded: TypedSpecPlan = serde_json::from_str(&emitted).unwrap();

    assert_eq!(
        serde_json::to_string_pretty(&decoded).unwrap(),
        emitted.trim_end()
    );
}
