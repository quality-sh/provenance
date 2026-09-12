//! Text safety: untrusted envelope text cannot create markup, mentions,
//! links, fences or workflow commands; truncation is explicit; an
//! operational failure never reads as clean; repository links are built
//! only from an immutable commit plus a validated path.

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

/// 96373e74a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6 → 52ccec3fb1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6, with the three new active Rules unverified.
fn scenario_a_envelope() -> Value {
    json!({
        "schema_version": 1,
        "repository": "quality-sh/provenance",
        "scope": "default",
        "base_commit": "96373e74a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6",
        "head_commit": "52ccec3fb1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6",
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

/// Count Markdown table delimiters that are not escaped with a backslash.
fn unescaped_pipes(row: &str) -> usize {
    let bytes = row.as_bytes();
    bytes
        .iter()
        .enumerate()
        .filter(|(i, b)| **b == b'|' && (*i == 0 || bytes[i - 1] != b'\\'))
        .count()
}

#[test]
#[verifies("rule_report_escapes_untrusted_text", examples)]
fn malicious_graph_text_cannot_create_markup_mentions_links_or_fences() {
    let nasty = "@coordinator see <script>alert(1)</script> and [click](https://evil.example) ![x](https://evil.example/i.png) ```fence``` a | b ::danger thing:: # not-a-heading";
    let mut envelope = scenario_a_envelope();
    envelope["graph_changes"][0]["statement"] = json!(nasty);
    envelope["findings"][0]["statement"] = json!(nasty);
    let report = render_markdown(&envelope);

    assert!(!report.contains("<script>"), "raw HTML must be escaped");
    assert!(
        report.contains("&lt;script&gt;"),
        "escaped HTML must stay visible as data"
    );
    assert!(
        !report.contains("@coordinator"),
        "mentions must be neutralized"
    );
    assert!(
        report.contains("@\u{200B}coordinator"),
        "mention neutralization must keep the name readable"
    );
    assert!(!report.contains("```"), "code fences must be impossible");
    assert!(
        report.contains("\\[click\\]"),
        "link brackets must be escaped"
    );
    assert!(
        !report.lines().any(|line| line.starts_with("::")),
        "workflow commands must never start a line"
    );
    assert!(
        !report
            .lines()
            .any(|line| line.starts_with("# not-a-heading")),
        "untrusted text must not create headings"
    );
    // The same text inside the graph-change table cannot add a column.
    let statement_row = report
        .lines()
        .find(|line| line.contains("not-a-heading"))
        .expect("statement must appear in the table");
    assert!(
        statement_row.contains("a \\| b"),
        "table cells must escape pipe delimiters"
    );
    assert_eq!(
        unescaped_pipes(statement_row),
        5,
        "table row must keep exactly five columns"
    );
}

#[test]
fn long_untrusted_text_is_truncated_explicitly_and_deterministically() {
    let long_statement = format!("{} end", "word ".repeat(80));
    let mut envelope = scenario_a_envelope();
    envelope["findings"][0]["statement"] = json!(long_statement);
    let report = render_markdown(&envelope);
    assert!(
        report.contains("[truncated; 204 characters omitted]"),
        "truncation must be explicit with the omitted count"
    );
    // Deterministic: the same envelope renders the same cut every time.
    let again = render_markdown(&envelope);
    assert_eq!(report, again);
}

#[test]
fn operational_failure_is_never_rendered_as_zero_findings() {
    let mut envelope = scenario_a_envelope();
    envelope["graph_changes"] = json!([]);
    envelope["findings"] = json!([]);
    envelope["verification_runs"] = json!([]);
    envelope["scan"]["failure"] = json!({
        "stage": "source_scan",
        "message": "walk abandoned after unreadable path"
    });
    let report = render_markdown(&envelope);
    assert!(
        report.contains("Operational failure at source scan"),
        "the failure must be rendered with its stage"
    );
    assert!(
        report.contains("findings are unavailable"),
        "empty findings under a failure must say unavailable, not clean"
    );
    assert!(
        !report.contains("0 findings"),
        "an operational failure cannot render as zero findings"
    );
}

#[test]
fn site_locations_render_as_repository_links_from_commit_and_path() {
    let mut envelope = scenario_a_envelope();
    let finding = envelope["findings"][0].as_object_mut().unwrap();
    finding.insert("binding_presence".into(), json!("present"));
    finding.insert(
        "sites".into(),
        json!([
            {
                "commit": "head",
                "path": "crates/provenance-core/src/model/validation.rs",
                "line": 19,
                "role": "verification",
                "method": "conformance"
            }
        ]),
    );
    let report = render_markdown(&envelope);
    let expected = "[`crates/provenance-core/src/model/validation.rs:19`](https://github.com/quality-sh/provenance/blob/52ccec3fb1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6/crates/provenance-core/src/model/validation.rs#L19)";
    assert!(
        report.contains(expected),
        "site must link to the immutable head commit and validated path"
    );
    assert!(
        report.contains("(method `conformance`)"),
        "the verification method must be labelled"
    );
}

#[test]
#[verifies("rule_report_escapes_untrusted_text", examples)]
fn untrusted_site_paths_never_become_repository_links() {
    let mut envelope = scenario_a_envelope();
    let finding = envelope["findings"][0].as_object_mut().unwrap();
    finding.insert("binding_presence".into(), json!("present"));
    finding.insert(
        "sites".into(),
        json!([
            { "commit": "head", "path": "../escape/evil.rs", "line": 1 },
            { "commit": "base", "path": "/absolute/path.rs", "line": 2 },
            { "commit": "head", "path": "back\\slash.rs", "line": 3 }
        ]),
    );
    let report = render_markdown(&envelope);
    assert!(
        !report.contains("](..") && !report.contains("](/"),
        "traversal or absolute paths must not become links"
    );
    assert!(
        !report.contains("blob/"),
        "no site may link when no path validates"
    );
    assert!(
        report.contains("`../escape/evil.rs:1`"),
        "invalid paths stay visible as plain text"
    );
    let again = render_markdown(&envelope);
    assert_eq!(report, again, "rejection must be deterministic");
}

#[test]
fn run_method_and_commit_are_escaped_before_interpolation() {
    let nasty_method = "examples) passed.\n\n## Injected run heading\n\n@evil-user do a thing";
    let mut envelope = scenario_a_envelope();
    envelope["verification_runs"] = json!([
        {
            "rule_id": "rule_active_rule_requires_verification",
            "method": nasty_method,
            "status": "passed",
            "commit": "52ccec3fb1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6"
        }
    ]);
    let report = render_markdown(&envelope);
    assert!(
        !report.contains("\n## Injected run heading"),
        "a run field must not create a heading"
    );
    assert!(
        !report.contains("@evil-user"),
        "a run field must not create a live mention"
    );
    assert!(
        report.contains("@\u{200B}evil-user"),
        "the mention must stay readable but neutralized"
    );
}

#[test]
fn bare_urls_in_untrusted_text_are_neutralized_against_autolinking() {
    let mut envelope = scenario_a_envelope();
    envelope["findings"][0]["statement"] =
        json!("see https://evil.example/x and www.phish.example now");
    let report = render_markdown(&envelope);
    assert!(
        !report.contains("https://evil.example"),
        "a bare https URL must not survive for GFM autolinking"
    );
    assert!(
        report.contains("https://\u{200B}evil.example"),
        "the scheme stays readable with the trigger broken"
    );
    assert!(
        !report.contains("www.phish.example"),
        "a bare www URL must not survive for GFM autolinking"
    );
    assert!(
        !report
            .lines()
            .any(|line| line.contains("(https://evil.example")),
        "no autolink destination may form"
    );
}

#[test]
fn envelope_scope_is_escaped_not_interpolated_raw() {
    let mut envelope = scenario_a_envelope();
    envelope["scope"] = json!("default\n\n## Injected scope heading\n\n@scope-user mention");
    let report = render_markdown(&envelope);
    assert!(
        !report.contains("\n## Injected scope heading"),
        "the scope must not create a heading"
    );
    assert!(
        !report.contains("@scope-user"),
        "the scope must not create a live mention"
    );
}

#[test]
fn site_removed_site_and_run_lists_are_bounded_with_explicit_omission() {
    let mut envelope = scenario_a_envelope();
    let finding = envelope["findings"][0].as_object_mut().unwrap();
    finding.insert("binding_presence".into(), json!("present"));
    let sites: Vec<Value> = (0..8)
        .map(|i| {
            json!({
                "commit": "head",
                "path": format!("src/site{i}.rs"),
                "line": i + 1,
                "role": "verification",
                "method": "examples"
            })
        })
        .collect();
    finding.insert("sites".into(), json!(sites));
    let removed: Vec<Value> = (0..7)
        .map(|i| {
            json!({
                "commit": "base",
                "path": format!("src/gone{i}.rs"),
                "line": i + 1
            })
        })
        .collect();
    finding.insert("removed_sites".into(), json!(removed));
    let runs: Vec<Value> = (0..7)
        .map(|i| {
            json!({
                "rule_id": "rule_active_rule_requires_verification",
                "method": "examples",
                "status": "passed",
                "commit": format!("aaaa{i}b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6fab")
            })
        })
        .collect();
    envelope["verification_runs"] = json!(runs);

    let report = render_markdown(&envelope);
    assert!(
        !report.contains("src/site7.rs"),
        "site lists must be bounded"
    );
    assert!(
        report.contains("3 more sites omitted"),
        "the site omission must be explicit"
    );
    assert!(
        !report.contains("src/gone6.rs"),
        "removed-site lists must be bounded"
    );
    assert!(
        report.contains("2 more removed sites omitted"),
        "the removed-site omission must be explicit"
    );
    assert!(!report.contains("aaaa6b1c2"), "run lists must be bounded");
    assert!(
        report.contains("2 more verification runs omitted"),
        "the run omission must be explicit"
    );
    let again = render_markdown(&envelope);
    assert_eq!(report, again, "list cuts must be deterministic");
}

#[test]
fn site_paths_with_unsafe_characters_never_link() {
    let mut envelope = scenario_a_envelope();
    let finding = envelope["findings"][0].as_object_mut().unwrap();
    finding.insert("binding_presence".into(), json!("present"));
    finding.insert(
        "sites".into(),
        json!([
            { "commit": "head", "path": "src/a`b.rs", "line": 1 },
            { "commit": "head", "path": "src/with space.rs", "line": 2 },
            { "commit": "head", "path": "src/paren(1).rs", "line": 3 },
            { "commit": "head", "path": "src/plain.rs", "line": 4 }
        ]),
    );
    let report = render_markdown(&envelope);
    assert!(
        !report.contains("](https://github.com/quality-sh/provenance/blob/52ccec3fb1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6/src/a"),
        "a path with a backtick must not link"
    );
    assert!(
        !report.contains("with%20space"),
        "unsafe paths are not linked, silently rewritten"
    );
    assert!(
        !report.contains("blob/52ccec3fb1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f/src/with"),
        "a path with spaces must not form a link destination"
    );
    assert!(
        !report.contains("blob/52ccec3fb1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f/src/paren"),
        "a path with parens must not form a link destination"
    );
    assert!(
        !report.contains("blob/52ccec3fb1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f/src/a"),
        "a path with a backtick must not form a link destination"
    );
    assert!(
        report.contains("`src/plain.rs:4`"),
        "safe paths still render"
    );
    let linked = report
        .matches("](https://github.com/quality-sh/provenance/blob/")
        .count();
    assert_eq!(linked, 1, "exactly the safe path links");
}
