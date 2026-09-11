//! Envelope validation: a rejected envelope never renders. An unknown
//! schema version or diagnostic code, an absence claimed by an
//! incomplete scan, new or resolved labels without a compatible
//! baseline, and a repository identity that is not owner/name are all
//! refused with a named diagnostic.

use assert_cmd::Command;
use serde_json::{json, Value};
use tempfile::TempDir;

/// Render and expect a non-zero exit with a diagnostic naming the problem.
fn render_rejected(envelope: &Value) -> String {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("envelope.json");
    std::fs::write(&input, serde_json::to_vec_pretty(envelope).unwrap()).unwrap();
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args(["report", "render", "--input"])
        .arg(&input)
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "envelope must be rejected; rendered: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    String::from_utf8(output.stderr).unwrap()
}

/// 96373e74 → 52ccec3f, with the three new active Rules unverified.
fn scenario_a_envelope() -> Value {
    json!({
        "schema_version": 1,
        "repository": "quality-sh/provenance",
        "scope": "default",
        "base_commit": "96373e74",
        "head_commit": "52ccec3f",
        "scan": {
            "completeness": "complete",
            "baseline": "compatible",
            "files_scanned": 214
        },
        "policy": { "mode": "warning", "result": "success" },
        "graph_changes": [
            {
                "kind": "requirement",
                "change": "added",
                "id": "req_rule_binding_findings_are_configurable",
                "statement": "Repository configuration selects warning or error severity for Rule binding findings.",
                "relations_added": [
                    { "relation": "refines", "target_kind": "requirement", "target_id": "req_build_a_static_analysis_scanner_in_the_s" }
                ]
            },
            {
                "kind": "rule",
                "change": "added",
                "id": "rule_binding_finding_uses_configured_severity",
                "statement": "The coverage check reports warning findings without failure and fails when an error finding occurs.",
                "relations_added": [
                    { "relation": "serves", "target_kind": "requirement", "target_id": "req_rule_binding_findings_are_configurable" },
                    { "relation": "produced_by", "target_kind": "resolution", "target_id": "res_rule_binding_checks_follow_lifecycle" }
                ]
            },
            {
                "kind": "rule",
                "change": "added",
                "id": "rule_active_rule_requires_verification",
                "statement": "The coverage check reports each active Rule that has no current verification binding.",
                "relations_added": [
                    { "relation": "serves", "target_kind": "requirement", "target_id": "req_rule_binding_findings_are_configurable" }
                ]
            },
            {
                "kind": "rule",
                "change": "added",
                "id": "rule_inactive_rules_have_no_current_bindings",
                "statement": "The coverage check reports current implementation or verification bindings to deprecated or archived Rules.",
                "relations_added": [
                    { "relation": "serves", "target_kind": "requirement", "target_id": "req_rule_binding_findings_are_configurable" }
                ]
            },
            {
                "kind": "resolution",
                "change": "added",
                "id": "res_rule_binding_checks_follow_lifecycle",
                "statement": "Check Rule bindings by lifecycle with configurable severity",
                "relations_added": [
                    { "relation": "resolves", "target_kind": "requirement", "target_id": "req_rule_binding_findings_are_configurable" }
                ]
            },
            {
                "kind": "source",
                "change": "added",
                "id": "source_rule_binding_policy_2026_09_11",
                "statement": "Ben: Rule binding checks by lifecycle, 11 September 2026"
            }
        ],
        "findings": [
            {
                "code": "active_rule_missing_verification",
                "subject": { "kind": "rule", "id": "rule_binding_finding_uses_configured_severity" },
                "severity": "warning",
                "comparison": "new",
                "binding_presence": "absent"
            },
            {
                "code": "active_rule_missing_verification",
                "subject": { "kind": "rule", "id": "rule_inactive_rules_have_no_current_bindings" },
                "severity": "warning",
                "comparison": "new",
                "binding_presence": "absent"
            },
            {
                "code": "active_rule_missing_verification",
                "subject": { "kind": "rule", "id": "rule_active_rule_requires_verification" },
                "severity": "warning",
                "comparison": "new",
                "binding_presence": "absent"
            }
        ],
        "verification_runs": []
    })
}

#[test]
fn unknown_schema_version_is_rejected_not_rendered() {
    let mut envelope = scenario_a_envelope();
    envelope["schema_version"] = json!(2);
    let message = render_rejected(&envelope);
    assert!(
        message.contains("schema"),
        "rejection must name the schema problem: {message}"
    );
}

#[test]
fn unknown_diagnostic_code_is_rejected_not_rendered() {
    let mut envelope = scenario_a_envelope();
    envelope["findings"][0]["code"] = json!("invented_code_from_graph_text");
    let message = render_rejected(&envelope);
    assert!(
        message.contains("invented_code_from_graph_text"),
        "rejection must name the unknown code: {message}"
    );
}

#[test]
fn incomplete_scan_cannot_establish_an_absence_finding() {
    let mut envelope = scenario_a_envelope();
    envelope["scan"]["completeness"] = json!("incomplete");
    envelope["scan"]["incompleteness_reason"] = json!("scan limited to crates/");
    for finding in envelope["findings"].as_array_mut().unwrap() {
        finding["binding_presence"] = json!("absent");
    }
    let message = render_rejected(&envelope);
    assert!(
        message.contains("incomplete"),
        "rejection must name the completeness limit: {message}"
    );
}

#[test]
fn missing_baseline_cannot_label_a_finding_new() {
    let mut envelope = scenario_a_envelope();
    envelope["scan"]["baseline"] = json!("missing");
    let message = render_rejected(&envelope);
    assert!(
        message.contains("baseline"),
        "rejection must name the baseline limit: {message}"
    );
}

#[test]
fn incompatible_baseline_cannot_label_a_finding_resolved() {
    let mut envelope = scenario_a_envelope();
    envelope["scan"]["baseline"] = json!("incompatible");
    envelope["scan"]["baseline_reason"] = json!("scanner revision differs");
    envelope["findings"][0]["comparison"] = json!("resolved");
    let message = render_rejected(&envelope);
    assert!(
        message.contains("baseline"),
        "rejection must name the baseline limit: {message}"
    );
}

#[test]
fn repository_identity_must_be_owner_slash_name() {
    let mut envelope = scenario_a_envelope();
    envelope["repository"] = json!("https://github.com/quality-sh/provenance");
    let message = render_rejected(&envelope);
    assert!(
        message.contains("repository"),
        "a full URL is not a repository identity: {message}"
    );
}
