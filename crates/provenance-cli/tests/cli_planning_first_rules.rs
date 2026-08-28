//! Reproduces the audit failure that a planning-first graph invited.
//!
//! An auditor met Rules that no code implemented, found no implementation
//! state on any agent-facing surface, and concluded that the agent had
//! invented them. Every Rule here is accepted and grounded in a Source and a
//! Requirement, and none of them has code. The surfaces an auditor reads must
//! say that in a word, and must not read the absence as a defect.

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

/// The word an auditor needs, on the command an auditor runs first.
#[test]
fn prime_reports_an_accepted_rule_with_no_code_as_unimplemented() {
    let dir = planning_first_repo();
    let repo = dir.path().to_string_lossy().to_string();

    let view: Value = serde_json::from_str(&provenance(&[
        "prime", "--repo", &repo, "--scope", "default", "--format", "json",
    ]))
    .unwrap();

    let rule = view["rules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|rule| rule["id"] == "rule_second_approver")
        .expect("prime lists the rule");
    assert_eq!(rule["implementation"], "unimplemented");
    // The record travels whole; the derived state joins it rather than
    // replacing what a reader already consumed.
    assert_eq!(rule["status"], "active");
    assert!(rule["statement"].is_string());
}

/// Prime says what the absence means in the same breath as reporting it. An
/// auditor who reads the state without the reading has learned nothing.
#[test]
fn prime_markdown_states_that_absence_is_not_invalidity() {
    let dir = planning_first_repo();
    let repo = dir.path().to_string_lossy().to_string();

    let rendered = provenance(&["prime", "--repo", &repo, "--scope", "default"]);

    assert!(
        rendered.contains("rule_second_approver (unimplemented)"),
        "{rendered}"
    );
    assert!(
        rendered.contains("A Rule can be active before code implements it."),
        "{rendered}"
    );
    assert!(
        rendered.contains("The absence of code is not evidence that a Rule is invalid."),
        "{rendered}"
    );
    for verdict in ["invented", "invalid Rule", "bogus", "fabricated"] {
        assert!(
            !rendered.contains(verdict),
            "prime called a planned Rule {verdict}: {rendered}"
        );
    }
}

/// Traceability answers both audit questions at once: the chain that grounds
/// the decision, and the separate fact that no code implements it yet.
#[test]
fn traceability_separates_the_grounding_chain_from_the_missing_code() {
    let dir = planning_first_repo();
    let repo = dir.path().to_string_lossy().to_string();

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

    assert_eq!(trace["implementation"], "unimplemented");
    // Decision fidelity holds: a Requirement and a Source stand behind it.
    // These are what an audit weighs, and code absence is not among them.
    assert_eq!(trace["requirements"][0]["id"], "req_second_approver");
    assert_eq!(trace["sources"][0]["id"], "source_finance_policy");
}

/// A Rule nobody has accepted is owed no code, so the report does not ask for
/// any. Calling this `unimplemented` would report work that nobody agreed to.
#[test]
fn a_draft_rule_is_not_reported_as_unimplemented() {
    let dir = planning_first_repo();
    let repo = dir.path().to_string_lossy().to_string();

    provenance(&[
        "rules",
        "create",
        "--repo",
        &repo,
        "--scope",
        "default",
        "--id",
        "rule_draft_idea",
        "--requirement-id",
        "req_second_approver",
        "--statement",
        "A drafted obligation that no human has accepted",
        "--status",
        "draft",
    ]);

    let view: Value = serde_json::from_str(&provenance(&[
        "prime", "--repo", &repo, "--scope", "default", "--format", "json",
    ]))
    .unwrap();

    let rules = view["rules"].as_array().unwrap();
    let draft = rules
        .iter()
        .find(|rule| rule["id"] == "rule_draft_idea")
        .unwrap();
    assert_eq!(draft["implementation"], "not_expected");
}

/// A Rule a Requirement produced needs no Resolution. The obsolete release
/// that demanded one is what taught agents to invent a decision to satisfy the
/// command.
#[test]
fn a_requirement_alone_can_produce_a_rule() {
    let dir = planning_first_repo();
    let repo = dir.path().to_string_lossy().to_string();

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

    assert!(
        trace["resolutions"].as_array().unwrap().is_empty(),
        "the rule cites no resolution, and none was invented for it"
    );
    assert_eq!(trace["requirements"][0]["id"], "req_second_approver");
}

/// The state is derived from the repository, not assumed. A scanner site in
/// the tree moves the same Rule to `implemented`, which is what makes the
/// `unimplemented` answer above worth reading.
#[test]
fn a_scanned_rule_site_moves_the_same_rule_to_implemented() {
    let dir = planning_first_repo();
    let repo = dir.path().to_string_lossy().to_string();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(
        dir.path().join("src/approval.rs"),
        "#[rule(\"rule_second_approver\")]\npub fn requires_second_approver() -> bool { true }\n",
    )
    .unwrap();

    let view: Value = serde_json::from_str(&provenance(&[
        "prime", "--repo", &repo, "--scope", "default", "--format", "json",
    ]))
    .unwrap();

    let rule = view["rules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|rule| rule["id"] == "rule_second_approver")
        .unwrap();
    assert_eq!(rule["implementation"], "implemented");
}
