//! Core golden tests for the deterministic report renderer: the scenario
//! A replay of the real PR 239 graph delta, byte-identical reordering,
//! stable ties and budget, and the two output forms. Scenario fixtures
//! come from the research Gist revision pinned in
//! `source_pr_review_feedback_research_2026_09_11`.

use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::{json, Value};
use tempfile::TempDir;

fn render_markdown(envelope: &Value) -> String {
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
        output.status.success(),
        "render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
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

/// Equivalent input must render byte-identical Markdown.
fn scenario_a_envelope_reversed() -> Value {
    let mut envelope = scenario_a_envelope();
    for key in ["graph_changes", "findings"] {
        let items = envelope[key].as_array().unwrap().clone();
        envelope[key] = Value::Array(items.into_iter().rev().collect());
    }
    let changes = envelope["graph_changes"].as_array_mut().unwrap();
    for change in changes.iter_mut() {
        if let Some(relations) = change.get_mut("relations_added") {
            let items = relations.as_array().unwrap().clone();
            *relations = Value::Array(items.into_iter().rev().collect());
        }
    }
    envelope
}

#[test]
fn scenario_a_graph_only_policy_change_names_added_obligations_in_stable_order() {
    let report = render_markdown(&scenario_a_envelope());

    // The comparison scope names both immutable commits.
    assert!(
        report.contains("96373e74 → 52ccec3f"),
        "report must state the comparison range"
    );
    // A useful group quotes the obligation as data.
    assert!(
        report.contains(
            "The coverage check reports each active Rule that has no current verification binding."
        ),
        "report must quote the added Rule statement"
    );
    // Every absence finding carries the prescribed next action from the catalog.
    assert_eq!(
        report.matches("Next action:").count(),
        3,
        "each of the three findings prescribes exactly one next action"
    );
    // Stable ordering: the Requirement row precedes the Rules; Rule ids sort.
    let req = report
        .find("req_rule_binding_findings_are_configurable")
        .unwrap();
    let r1 = report
        .find("rule_active_rule_requires_verification")
        .unwrap();
    let r2 = report
        .find("rule_binding_finding_uses_configured_severity")
        .unwrap();
    let r3 = report
        .find("rule_inactive_rules_have_no_current_bindings")
        .unwrap();
    assert!(req < r1 && r1 < r2 && r2 < r3, "ordering must be stable");
}

#[test]
#[verifies("rule_report_render_is_deterministic", examples)]
fn reordered_equivalent_input_renders_byte_identical_markdown() {
    let straight = render_markdown(&scenario_a_envelope());
    let reversed = render_markdown(&scenario_a_envelope_reversed());
    assert_eq!(straight, reversed, "input order must not change the bytes");
}

#[test]
fn identical_identities_break_ties_by_site_and_ignore_input_order() {
    let mut envelope = scenario_a_envelope();
    envelope["graph_changes"] = json!([]);
    let base_finding = json!({
        "code": "verification_site_moved",
        "subject": { "kind": "rule", "id": "rule_confidence_range" },
        "severity": "warning",
        "comparison": "new",
        "binding_presence": "present"
    });
    let mut first = base_finding.clone();
    first["sites"] = json!([{ "commit": "head", "path": "a/first.rs", "line": 10 }]);
    let mut second = base_finding;
    second["sites"] = json!([{ "commit": "head", "path": "b/second.rs", "line": 20 }]);
    envelope["findings"] = json!([second, first]);
    let one = render_markdown(&envelope);
    envelope["findings"] = json!([first, second]);
    let other = render_markdown(&envelope);
    assert_eq!(one, other, "ties must not depend on input order");
    let a = one.find("a/first.rs").unwrap();
    let b = one.find("b/second.rs").unwrap();
    assert!(a < b, "site path decides the tie");
}

#[test]
fn findings_beyond_the_display_budget_are_omitted_explicitly() {
    let mut envelope = scenario_a_envelope();
    let mut findings = Vec::new();
    for i in 0..12 {
        findings.push(json!({
            "code": "active_rule_missing_verification",
            "subject": { "kind": "rule", "id": format!("rule_{i:02}") },
            "severity": "warning",
            "comparison": "new",
            "binding_presence": "absent"
        }));
    }
    envelope["findings"] = json!(findings);
    let report = render_markdown(&envelope);
    assert!(
        !report.contains("rule_11"),
        "an omitted finding must not render at all"
    );
    assert!(
        report.contains("2 more findings omitted."),
        "the omission must be explicit with the count"
    );
    let again = render_markdown(&envelope);
    assert_eq!(report, again, "budget cuts must be deterministic");
}

#[test]
fn structured_json_output_gives_a_publisher_normalized_facts_not_markdown() {
    let reversed = scenario_a_envelope_reversed();
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("envelope.json");
    std::fs::write(&input, serde_json::to_vec_pretty(&reversed).unwrap()).unwrap();
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args(["report", "render", "--input"])
        .arg(&input)
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        !stdout.contains("# Provenance report"),
        "structured output is not markdown for publishers to scrape"
    );
    let normalized: Value = serde_json::from_str(&stdout).unwrap();
    let ids: Vec<&str> = normalized["graph_changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|change| change["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "req_rule_binding_findings_are_configurable",
            "rule_active_rule_requires_verification",
            "rule_binding_finding_uses_configured_severity",
            "rule_inactive_rules_have_no_current_bindings",
            "res_rule_binding_checks_follow_lifecycle",
            "source_rule_binding_policy_2026_09_11"
        ],
        "json output carries canonical kind-first order regardless of input order"
    );
}

#[test]
fn output_flag_writes_the_report_to_a_file() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("envelope.json");
    let output_path = dir.path().join("report.md");
    std::fs::write(
        &input,
        serde_json::to_vec_pretty(&scenario_a_envelope()).unwrap(),
    )
    .unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["report", "render", "--input"])
        .arg(&input)
        .args(["--output"])
        .arg(&output_path)
        .output()
        .unwrap();
    let written = std::fs::read_to_string(&output_path).unwrap();
    assert!(written.starts_with("# Provenance report"));
    assert!(written.ends_with("no language model writes this report.\n"));
}
