//! The lifecycle binding policy through the real CLI: configuration selects
//! warning or error, and the full-repository scan reports the findings.

use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn run(repo: &Path, args: &[&str]) {
    provenance().args(args).current_dir(repo).assert().success();
}

fn init_repo() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    run(
        directory.path(),
        &[
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ],
    );
    run(
        directory.path(),
        &[
            "requirements",
            "create",
            "--repo",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            "req_anchor",
            "--statement",
            "The anchor requirement holds",
        ],
    );
    directory
}

fn create_rule(repo: &Path, id: &str, status: &str) {
    run(
        repo,
        &[
            "rules",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            id,
            "--requirement-id",
            "req_anchor",
            "--statement",
            "The covered obligation holds",
            "--status",
            status,
            "--severity",
            "high",
        ],
    );
}

fn set_binding_policy(repo: &Path, severity: &str) {
    let settings = repo.join(".provenance/settings.json");
    std::fs::write(
        &settings,
        json!({"coverage": {"binding_findings": severity}}).to_string(),
    )
    .unwrap();
}

/// A whole-repository scan: absence findings need the root as the path.
fn full_scan(repo: &Path) -> Command {
    let mut command = provenance();
    command.args([
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
    ]);
    command
}

fn warnings(stdout: &[u8]) -> Vec<Value> {
    let report: Value = serde_json::from_slice(stdout).unwrap();
    report["warnings"]
        .as_array()
        .expect("warnings array")
        .clone()
}

fn begin_verification(repo: &Path, rule: &str, key: &str, file: &str) {
    let path = repo.join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "// exercises the rule\n").unwrap();
    provenance()
        .args([
            "sdk",
            "begin-verification",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .write_stdin(
            serde_json::to_vec(&json!({
                "rule": rule,
                "key": key,
                "method": "examples",
                "declared_by": "ci://binding-policy-test",
                "file": file
            }))
            .unwrap(),
        )
        .assert()
        .success();
}

/// One spec-owned Rule the retirement document later withdraws.
fn spec_document(owner: &str) -> Value {
    json!({
        "schema_version": 2,
        "spec": "retirement",
        "declared_by": owner,
        "sources": [],
        "requirements": [{
            "key": "withdrawn",
            "statement": "The withdrawn obligation holds",
            "sources": []
        }],
        "rules": [{
            "key": "withdrawn",
            "requirement": "withdrawn",
            "statement": "The covered obligation holds"
        }]
    })
}

fn empty_spec_document(owner: &str) -> Value {
    json!({
        "schema_version": 2,
        "spec": "retirement",
        "declared_by": owner,
        "sources": [],
        "requirements": [],
        "rules": []
    })
}

fn apply_spec(repo: &Path, document: &Value) -> Value {
    let output = provenance()
        .args([
            "sdk",
            "apply",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .write_stdin(serde_json::to_vec(document).unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn spec_rule_id(applied: &Value) -> String {
    applied["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == "rule" && resource["key"] == "withdrawn")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// A current typed verification binding to a Rule the graph later retires:
/// the binding was created while the Rule stood, and the withdrawal leaves
/// the binding in place.
fn retire_a_rule_with_a_current_verification(repo: &Path) {
    let owner = "spec://tests/retirement";
    let applied = apply_spec(repo, &spec_document(owner));
    let rule_id = spec_rule_id(&applied);
    begin_verification(repo, &rule_id, "withdrawn-check", "tests/withdrawn.test.ts");
    apply_spec(repo, &empty_spec_document(owner));
}

#[test]
#[verifies("rule_inactive_rules_have_no_current_bindings", examples)]
fn the_warning_policy_reports_a_current_binding_to_a_retired_rule_without_failing() {
    let repo = init_repo();
    retire_a_rule_with_a_current_verification(repo.path());

    let output = full_scan(repo.path()).output().unwrap();

    assert!(output.status.success());
    let finding = warnings(&output.stdout)
        .into_iter()
        .find(|warning| {
            warning["rule_id"]
                .as_str()
                .unwrap_or("")
                .starts_with("rule_")
        })
        .expect("the current binding to a retired rule must be reported");
    assert_eq!(finding["binding_finding"], json!(true));
    assert!(
        finding["message"]
            .as_str()
            .is_some_and(|message| message.contains("retired")),
        "{finding}"
    );
}

#[test]
#[verifies("rule_binding_finding_uses_configured_severity", examples)]
fn the_error_policy_fails_on_a_current_binding_to_a_retired_rule_after_printing_the_report() {
    let repo = init_repo();
    retire_a_rule_with_a_current_verification(repo.path());
    set_binding_policy(repo.path(), "error");

    let output = full_scan(repo.path()).output().unwrap();

    assert!(!output.status.success());
    let finding = warnings(&output.stdout)
        .into_iter()
        .find(|warning| warning["binding_finding"] == json!(true))
        .expect("the report still prints before the failure");
    assert!(
        finding["message"]
            .as_str()
            .is_some_and(|message| message.contains("retired")),
        "{finding}"
    );
}

#[test]
#[verifies("rule_inactive_rules_have_no_current_bindings", examples)]
fn the_error_policy_passes_for_a_retired_rule_without_current_bindings() {
    let repo = init_repo();
    let owner = "spec://tests/retirement";
    apply_spec(repo.path(), &spec_document(owner));
    apply_spec(repo.path(), &empty_spec_document(owner));
    set_binding_policy(repo.path(), "error");

    full_scan(repo.path()).assert().success();
}

#[test]
#[verifies("rule_inactive_rules_have_no_current_bindings", examples)]
fn a_retired_binding_to_a_retired_rule_is_not_current_evidence() {
    let repo = init_repo();
    let owner = "spec://tests/retirement";
    let applied = apply_spec(repo.path(), &spec_document(owner));
    let retired_rule = spec_rule_id(&applied);
    begin_verification(
        repo.path(),
        &retired_rule,
        "withdrawn-check",
        "tests/withdrawn.test.ts",
    );
    // The same owner, file, and key now vouch for the standing rule, so the
    // binding to the spec rule retires in place before the rule retires.
    create_rule(repo.path(), "rule_standing", "active");
    begin_verification(
        repo.path(),
        "rule_standing",
        "withdrawn-check",
        "tests/withdrawn.test.ts",
    );
    apply_spec(repo.path(), &empty_spec_document(owner));
    set_binding_policy(repo.path(), "error");

    let output = full_scan(repo.path()).output().unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
#[verifies("rule_active_rule_requires_verification", examples)]
fn the_default_policy_reports_an_unverified_active_rule_without_failing() {
    let repo = init_repo();
    create_rule(repo.path(), "rule_uncovered", "active");

    let output = full_scan(repo.path()).output().unwrap();

    assert!(output.status.success());
    let findings = warnings(&output.stdout);
    let finding = findings
        .iter()
        .find(|warning| {
            warning["rule_id"] == "rule_uncovered"
                && warning["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("has no verification"))
        })
        .expect("the unverified active rule must be reported");
    assert_eq!(finding["binding_finding"], json!(true));
}

#[test]
#[verifies("rule_binding_finding_uses_configured_severity", examples)]
fn the_error_policy_fails_on_an_unverified_active_rule_after_printing_the_report() {
    let repo = init_repo();
    create_rule(repo.path(), "rule_uncovered", "active");
    set_binding_policy(repo.path(), "error");

    let output = full_scan(repo.path()).output().unwrap();

    assert!(!output.status.success());
    let findings = warnings(&output.stdout);
    assert!(
        findings
            .iter()
            .any(|warning| warning["rule_id"] == "rule_uncovered"),
        "the report still prints before the failure: {findings:#?}"
    );
}

#[test]
#[verifies("rule_active_rule_requires_verification", examples)]
fn the_error_policy_passes_once_the_active_rule_has_a_verification_site() {
    let repo = init_repo();
    create_rule(repo.path(), "rule_uncovered", "active");
    set_binding_policy(repo.path(), "error");
    std::fs::write(
        repo.path().join("src_check.rs"),
        "#[verifies(\"rule_uncovered\", examples)]\nfn checks_the_rule() {}\n",
    )
    .unwrap();

    full_scan(repo.path()).assert().success();
}

#[test]
#[verifies("rule_inactive_rules_have_no_current_bindings", examples)]
fn the_error_policy_fails_on_a_deprecated_rule_with_a_current_typed_verification() {
    let repo = init_repo();
    create_rule(repo.path(), "rule_old_gate", "deprecated");
    begin_verification(
        repo.path(),
        "rule_old_gate",
        "gate-check",
        "tests/old_gate.test.ts",
    );
    set_binding_policy(repo.path(), "error");

    let output = full_scan(repo.path()).output().unwrap();

    assert!(!output.status.success());
    let findings = warnings(&output.stdout);
    let finding = findings
        .iter()
        .find(|warning| warning["rule_id"] == "rule_old_gate")
        .expect("the current binding to a deprecated rule must be reported");
    assert_eq!(finding["binding_finding"], json!(true));
    assert!(
        finding["message"]
            .as_str()
            .is_some_and(|message| message.contains("deprecated")),
        "{finding}"
    );
}

#[test]
#[verifies("rule_inactive_rules_have_no_current_bindings", examples)]
fn the_error_policy_passes_for_a_deprecated_rule_without_current_bindings() {
    let repo = init_repo();
    create_rule(repo.path(), "rule_old_gate", "deprecated");
    set_binding_policy(repo.path(), "error");

    full_scan(repo.path()).assert().success();
}

#[test]
#[verifies("rule_inactive_rules_have_no_current_bindings", examples)]
fn a_retired_typed_verification_does_not_fail_the_error_policy() {
    let repo = init_repo();
    create_rule(repo.path(), "rule_old_gate", "deprecated");
    create_rule(repo.path(), "rule_new_gate", "active");
    begin_verification(
        repo.path(),
        "rule_old_gate",
        "gate-check",
        "tests/gate.test.ts",
    );
    // The same owner, file, and key now vouch for the new rule, so the
    // binding to the deprecated rule retires in place.
    begin_verification(
        repo.path(),
        "rule_new_gate",
        "gate-check",
        "tests/gate.test.ts",
    );
    set_binding_policy(repo.path(), "error");

    let output = full_scan(repo.path()).output().unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
#[verifies("rule_binding_finding_uses_configured_severity", examples)]
fn strict_still_fails_on_every_warning_under_the_warning_policy() {
    let repo = init_repo();
    create_rule(repo.path(), "rule_uncovered", "active");

    let output = full_scan(repo.path()).arg("--strict").output().unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--strict"));
}

#[test]
fn invalid_settings_refuse_the_scan_instead_of_falling_back() {
    let repo = init_repo();
    create_rule(repo.path(), "rule_uncovered", "active");
    std::fs::write(
        repo.path().join(".provenance/settings.json"),
        json!({"coverage": {"binding_findings": "blocking"}}).to_string(),
    )
    .unwrap();

    full_scan(repo.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("coverage.binding_findings"));
}

#[test]
#[verifies("rule_binding_finding_uses_configured_severity", examples)]
fn an_explicit_warning_setting_behaves_like_the_default() {
    let repo = init_repo();
    create_rule(repo.path(), "rule_uncovered", "active");
    set_binding_policy(repo.path(), "warning");

    let output = full_scan(repo.path()).output().unwrap();

    assert!(output.status.success());
    assert!(
        warnings(&output.stdout)
            .iter()
            .any(|warning| warning["rule_id"] == "rule_uncovered"),
        "the finding stays visible"
    );
}
