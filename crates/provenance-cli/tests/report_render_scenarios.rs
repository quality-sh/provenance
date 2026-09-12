//! Research scenarios B-F (mockups in the pinned Gist revision): an
//! active Rule without verification, a retired Rule with current
//! bindings, a removed verification site, recovery after a prior
//! finding, and a Requirement restatement with untouched bindings.

use assert_cmd::Command;
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

/// Scenario B: an active Rule without current verification under an error
/// policy. No graph change; the finding comes from the coverage scan.
fn scenario_b_envelope() -> Value {
    json!({
        "schema_version": 1,
        "repository": "quality-sh/provenance",
        "scope": "default",
        "base_commit": "e898da54c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6",
        "head_commit": "4e84a14cd1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6",
        "scan": { "completeness": "complete", "baseline": "compatible", "files_scanned": 214 },
        "policy": { "mode": "error", "result": "failure" },
        "findings": [
            {
                "code": "active_rule_missing_verification",
                "subject": { "kind": "rule", "id": "rule_active_rule_requires_verification" },
                "severity": "error",
                "comparison": "pre_existing",
                "binding_presence": "absent",
                "statement": "The coverage check reports each active Rule that has no current verification binding.",
                "affected_requirement_id": "req_rule_binding_findings_are_configurable"
            }
        ],
        "verification_runs": []
    })
}

#[test]
fn scenario_b_active_rule_without_verification_states_unknowns_and_failure() {
    let report = render_markdown(&scenario_b_envelope());
    assert!(
        report.contains("### error: no current verification binding found"),
        "the error finding carries its severity in the heading"
    );
    assert!(
        report.contains("Result: failure; the check fails."),
        "the configured error result must be stated"
    );
    assert!(
        report.contains("- Current bindings: none."),
        "absent binding presence must be stated"
    );
    assert!(
        report.contains("Verification run: not supplied."),
        "the missing run fact must be stated, not omitted"
    );
    assert!(
        report.contains("Refines: `req_rule_binding_findings_are_configurable`"),
        "the affected Requirement must be named"
    );
    assert!(!report.contains("## Graph changes"));
}

/// Scenario C (hypothetical H1): the real `rule_confidence_range` moves from
/// active to deprecated while its real source sites stay unchanged.
fn scenario_c_envelope() -> Value {
    json!({
        "schema_version": 1,
        "repository": "quality-sh/provenance",
        "scope": "default",
        "base_commit": "e898da54c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6",
        "head_commit": "1a2b3c4de1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6",
        "scan": { "completeness": "complete", "baseline": "compatible", "files_scanned": 214 },
        "policy": { "mode": "warning", "result": "success" },
        "graph_changes": [
            {
                "kind": "rule",
                "change": "changed",
                "id": "rule_confidence_range",
                "statement": "A confidence is a real number between zero and one; out-of-range values are never silently repaired",
                "statement_before": "A confidence is a real number between zero and one; out-of-range values are never silently repaired",
                "lifecycle_before": "active",
                "lifecycle_after": "deprecated"
            }
        ],
        "findings": [
            {
                "code": "inactive_rule_current_binding",
                "subject": { "kind": "rule", "id": "rule_confidence_range" },
                "severity": "warning",
                "comparison": "pre_existing",
                "binding_presence": "present",
                "statement": "A confidence is a real number between zero and one; out-of-range values are never silently repaired",
                "sites": [
                    {
                        "commit": "head",
                        "path": "crates/provenance-core/src/model/validation.rs",
                        "line": 19,
                        "role": "implementation"
                    },
                    {
                        "commit": "head",
                        "path": "crates/provenance-core/src/model/validation/tests.rs",
                        "line": 66,
                        "role": "verification",
                        "method": "conformance"
                    }
                ]
            }
        ],
        "verification_runs": []
    })
}

#[test]
fn scenario_c_deprecated_rule_with_current_bindings_reports_lifecycle_change() {
    let report = render_markdown(&scenario_c_envelope());
    assert!(
        report.contains("status: active → deprecated"),
        "the lifecycle transition must be shown in the graph changes"
    );
    assert!(
        report.contains("### warning: a retired Rule still has current bindings"),
        "the finding headline comes from the catalog"
    );
    assert!(
        report.contains(
            "[`crates/provenance-core/src/model/validation.rs:19`](https://github.com/quality-sh/provenance/blob/1a2b3c4de1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6/crates/provenance-core/src/model/validation.rs#L19)"
        ),
        "the current implementation site links to the immutable head commit"
    );
    assert!(
        report.contains("Current verification:"),
        "current verification evidence must be named"
    );
    assert!(
        report.contains("(method `conformance`)"),
        "the verification method must be labelled"
    );
    assert!(
        report.contains("Result: success; the check passes."),
        "warning mode reports and succeeds"
    );
}

/// Scenario D (hypothetical H2): the real conformance site for
/// `rule_confidence_range` is gone while core property evidence remains.
fn scenario_d_envelope() -> Value {
    json!({
        "schema_version": 1,
        "repository": "quality-sh/provenance",
        "scope": "default",
        "base_commit": "e898da54c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6",
        "head_commit": "2b3c4d5ef1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6",
        "scan": { "completeness": "complete", "baseline": "compatible", "files_scanned": 214 },
        "policy": { "mode": "warning", "result": "success" },
        "findings": [
            {
                "code": "verification_site_removed",
                "subject": { "kind": "rule", "id": "rule_confidence_range" },
                "severity": "warning",
                "comparison": "new",
                "binding_presence": "present",
                "statement": "A confidence is a real number between zero and one; out-of-range values are never silently repaired",
                "sites": [
                    {
                        "commit": "head",
                        "path": "crates/provenance-core/src/model/confidence.rs",
                        "line": 40,
                        "role": "verification",
                        "method": "property"
                    }
                ],
                "removed_sites": [
                    {
                        "commit": "base",
                        "path": "crates/provenance-core/src/model/validation/tests.rs",
                        "line": 66,
                        "role": "verification",
                        "method": "conformance"
                    }
                ]
            }
        ],
        "verification_runs": []
    })
}

#[test]
fn scenario_d_removed_site_shows_lost_evidence_and_surviving_evidence() {
    let report = render_markdown(&scenario_d_envelope());
    assert!(
        report.contains("### warning: one verification site is gone"),
        "the removal headline comes from the catalog"
    );
    let expected_removed = "[`crates/provenance-core/src/model/validation/tests.rs:66`](https://github.com/quality-sh/provenance/blob/e898da54c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6/crates/provenance-core/src/model/validation/tests.rs#L66)";
    assert!(
        report.contains(&format!(
            "- Removed evidence: {expected_removed} (method `conformance`)."
        )),
        "removed evidence links to the immutable base commit"
    );
    assert!(
        report.contains("Current verification:"),
        "surviving evidence must still be shown"
    );
    assert!(
        !report.contains("Current bindings: none."),
        "a removed site with surviving evidence is not an absence finding"
    );
    assert!(
        report.contains("Restore the removed check"),
        "the prescribed next action comes from the catalog"
    );
}

/// Scenario E (hypothetical H3): the exact conformance site is restored, so
/// the earlier finding is resolved in a compatible complete scan.
#[test]
fn scenario_e_resolved_finding_reports_recovery_not_approval() {
    let envelope = json!({
        "schema_version": 1,
        "repository": "quality-sh/provenance",
        "scope": "default",
        "base_commit": "2b3c4d5ef1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6",
        "head_commit": "3c4d5e6fa1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6",
        "scan": { "completeness": "complete", "baseline": "compatible", "files_scanned": 214 },
        "policy": { "mode": "warning", "result": "success" },
        "findings": [
            {
                "code": "verification_site_removed",
                "subject": { "kind": "rule", "id": "rule_confidence_range" },
                "severity": "warning",
                "comparison": "resolved",
                "binding_presence": "present",
                "statement": "A confidence is a real number between zero and one; out-of-range values are never silently repaired",
                "sites": [
                    {
                        "commit": "head",
                        "path": "crates/provenance-core/src/model/validation/tests.rs",
                        "line": 66,
                        "role": "verification",
                        "method": "conformance"
                    }
                ]
            }
        ],
        "verification_runs": []
    });
    let report = render_markdown(&envelope);
    assert!(
        report.contains("1 finding: 1 resolved finding; shown."),
        "the summary counts the recovery"
    );
    assert!(
        report.contains("resolved in this comparison; absent at the head"),
        "the resolved label must be stated"
    );
    assert!(
        report.contains("Verification run: not supplied."),
        "recovery confirms the binding, not a successful test"
    );
}

/// Scenario F: the real confidence Requirement is restated while its Rule
/// and bindings stay untouched. A new warning is present to pin ordering.
#[test]
fn scenario_f_restatement_names_the_rule_and_orders_intent_first() {
    let envelope = json!({
        "schema_version": 1,
        "repository": "quality-sh/provenance",
        "scope": "default",
        "base_commit": "e898da54c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6",
        "head_commit": "4d5e6f70b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6",
        "scan": { "completeness": "complete", "baseline": "compatible", "files_scanned": 214 },
        "policy": { "mode": "warning", "result": "success" },
        "graph_changes": [
            {
                "kind": "requirement",
                "change": "changed",
                "id": "req_confidence_keeps_authored_score",
                "statement": "A confidence outside the interval from zero to 0.9 is rejected.",
                "statement_before": "A confidence outside the finite unit interval is never silently clamped"
            }
        ],
        "findings": [
            {
                "code": "requirement_statement_changed",
                "subject": { "kind": "requirement", "id": "req_confidence_keeps_authored_score" },
                "severity": "warning",
                "comparison": "new",
                "binding_presence": "unknown",
                "statement": "A confidence outside the interval from zero to 0.9 is rejected.",
                "relevance": "review_requested",
                "affected_rule_id": "rule_confidence_range"
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
    });
    let report = render_markdown(&envelope);
    assert!(
        report.contains(
            "A confidence outside the finite unit interval is never silently clamped → A confidence outside the interval from zero to 0.9 is rejected."
        ),
        "the statement transition must show before and after"
    );
    assert!(
        report.contains("The Requirement says:"),
        "the restated obligation is quoted as data"
    );
    assert!(
        report.contains("Affected Rule: `rule_confidence_range`"),
        "the affected Rule must be named"
    );
    assert!(
        report.contains("Evidence relevance: a Requirement review asks for new evidence."),
        "the review request is a separate fact"
    );
    let warning = report
        .find("rule_active_rule_requires_verification")
        .unwrap();
    let restatement = report.find("rule_confidence_range").unwrap();
    assert!(
        warning < restatement,
        "new warnings render before changed-intent reviews"
    );
}
