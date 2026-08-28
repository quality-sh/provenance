//! Reproduces the audit failure that a planning-first graph invited.
//!
//! An auditor met a Rule that no code implemented and concluded that the agent
//! had invented it. The graph reads must preserve its lifecycle and grounding
//! without pretending to have scanned code. Coverage remains the command that
//! evaluates implementation and verification bindings.

use assert_cmd::Command;
use serde_json::Value;

/// A Rule that a Source and a Requirement support, and that no code
/// implements. This is the shape the auditor called invented.
fn planning_first_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_string_lossy().to_string();

    provenance(&[
        "init",
        "--path",
        &repo,
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ]);
    provenance(&[
        "sources",
        "create",
        "--repo",
        &repo,
        "--scope",
        "default",
        "--id",
        "source_finance_policy",
        "--name",
        "Finance Policy v3, section 4.2",
    ]);
    provenance(&[
        "requirements",
        "create",
        "--repo",
        &repo,
        "--scope",
        "default",
        "--id",
        "req_second_approver",
        "--statement",
        "Expenses above the delegated authority limit need second-approver sign-off",
    ]);
    provenance(&[
        "requirements",
        "source-ref",
        "add",
        "--repo",
        &repo,
        "--scope",
        "default",
        "--requirement-id",
        "req_second_approver",
        "--source-id",
        "source_finance_policy",
    ]);
    provenance(&[
        "rules",
        "create",
        "--repo",
        &repo,
        "--scope",
        "default",
        "--id",
        "rule_second_approver",
        "--requirement-id",
        "req_second_approver",
        "--statement",
        "An expense above the delegated authority limit needs a second approver",
        "--severity",
        "high",
    ]);
    dir
}

fn provenance(args: &[&str]) -> String {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8(output).unwrap()
}

/// Graph reads show the accepted, grounded Rule without a code verdict. The
/// canonical coverage scan separately reports that its implementation binding
/// is absent.
#[test]
fn planning_first_rule_is_grounded_before_code_exists() {
    let dir = planning_first_repo();
    let repo = dir.path().to_string_lossy().to_string();

    let prime: Value = serde_json::from_str(&provenance(&[
        "prime", "--repo", &repo, "--scope", "default", "--format", "json",
    ]))
    .unwrap();
    let rule = prime["rules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|rule| rule["id"] == "rule_second_approver")
        .expect("prime lists the rule");
    assert_eq!(rule["status"], "active");
    assert!(rule.get("implementation").is_none());

    let trace: Value = serde_json::from_str(&provenance(&[
        "traceability",
        "rule_second_approver",
        "--repo",
        &repo,
        "--scope",
        "default",
        "--format",
        "json",
    ]))
    .unwrap();

    assert!(trace.get("implementation").is_none());
    assert_eq!(trace["requirements"][0]["id"], "req_second_approver");
    assert_eq!(trace["sources"][0]["id"], "source_finance_policy");
    assert!(
        trace["resolutions"].as_array().unwrap().is_empty(),
        "the rule cites no resolution, and none was invented for it"
    );

    let coverage: Value = serde_json::from_str(&provenance(&[
        "coverage",
        "scan",
        "--repo",
        &repo,
        "--path",
        &repo,
        "--scope",
        "default",
        "--validate-rules",
        "--format",
        "json",
    ]))
    .unwrap();
    let warnings = coverage["warnings"].as_array().unwrap();
    assert!(
        warnings.iter().any(|warning| {
            warning["rule_id"] == "rule_second_approver"
                && warning["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("has no implementation"))
        }),
        "{coverage}"
    );
}
