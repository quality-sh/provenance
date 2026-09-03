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

fn invalid_spec() -> Value {
    json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "feedback",
        "declared_by": "spec://typescript/feedback",
        "requirements": [{
            "key": "sharing",
            "statement": "Café; requirement"
        }],
        "rules": [{
            "key": "expiry",
            "requirement": "sharing",
            "statement": "Rule; statement"
        }]
    })
}

fn sdk_output(repo: &str, command: &str) -> std::process::Output {
    provenance()
        .args([
            "sdk", command, "--repo", repo, "--scope", "default", "--format", "json",
        ])
        .write_stdin(serde_json::to_vec(&invalid_spec()).unwrap())
        .output()
        .unwrap()
}

#[test]
#[verifies("rule_ste_typed_diagnostic_parity", examples)]
fn sdk_plan_emits_machine_readable_typed_diagnostics() {
    let directory = init_repo();
    let output = sdk_output(directory.path().to_str().unwrap(), "plan");
    assert!(output.status.success());
    let plan: Value = serde_json::from_slice(&output.stdout).unwrap();

    assert_eq!(plan["diagnostics"].as_array().unwrap().len(), 2);
    assert_eq!(
        plan["diagnostics"][0],
        json!({
            "address": ["feedback", "requirement", "sharing"],
            "resource_kind": "requirement",
            "field": "statement",
            "standard": "ASD-STE100",
            "issue": 9,
            "rule": "8.1",
            "disposition": "violation",
            "span": {"start": 5, "end": 6},
            "message": "Do not use semicolons in descriptive text."
        })
    );
}

#[test]
#[verifies("rule_ste_typed_diagnostic_parity", examples)]
#[verifies("rule_ste_typed_apply_atomic_rejection", examples)]
fn sdk_apply_exits_nonzero_with_the_same_structured_diagnostics() {
    let directory = init_repo();
    let repo = directory.path().to_str().unwrap();
    let plan_output = sdk_output(repo, "plan");
    let plan: Value = serde_json::from_slice(&plan_output.stdout).unwrap();

    let apply_output = sdk_output(repo, "apply");

    assert!(!apply_output.status.success());
    let stderr = String::from_utf8(apply_output.stderr).unwrap();
    let report: Value = serde_json::from_str(
        stderr
            .trim()
            .strip_prefix("Error: ")
            .expect("CLI errors retain the serialized typed report"),
    )
    .unwrap();
    assert_eq!(report["error"], "asd_ste100_violations");
    assert_eq!(report["diagnostics"], plan["diagnostics"]);

    let export = provenance()
        .args([
            "export", "--repo", repo, "--scope", "default", "--format", "json",
        ])
        .output()
        .unwrap();
    assert!(export.status.success());
    let graph: Value = serde_json::from_slice(&export.stdout).unwrap();
    assert!(graph["requirements"].as_array().unwrap().is_empty());
    assert!(graph["rules"].as_array().unwrap().is_empty());
}
