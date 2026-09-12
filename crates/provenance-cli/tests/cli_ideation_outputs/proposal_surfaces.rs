use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use provenance_macros::verifies;

use super::support::init_repo;

fn provenance(repo: &str, args: &[&str]) -> assert_cmd::assert::Assert {
    let mut command = Command::cargo_bin("provenance").unwrap();
    command.args(args).args(["--repo", repo]);
    command.assert()
}

fn create_requirement_topic_and_proposal(repo: &str) {
    provenance(
        repo,
        &[
            "requirements",
            "create",
            "--scope",
            "default",
            "--id",
            "req_overtime",
            "--statement",
            "Overtime must be correct",
        ],
    )
    .success();
    provenance(
        repo,
        &[
            "topics",
            "create",
            "--scope",
            "default",
            "--id",
            "topic_overtime",
            "--requirement-id",
            "req_overtime",
            "--title",
            "Overtime",
        ],
    )
    .success();
    provenance(
        repo,
        &[
            "proposals",
            "create",
            "--scope",
            "default",
            "--id",
            "proposal_overtime",
            "--proposal-key",
            "overtime",
            "--proposal-type",
            "requirement_candidate",
            "--title",
            "Clarify overtime",
            "--summary",
            "Observed behavior",
            "--target-type",
            "requirement",
            "--target-id",
            "req_overtime",
            "--evidence-json",
            r#"[{"reference_id":"evidence_overtime","evidence_type":"artifact","summary":"implementation","file_path":"src/payroll.rs","line":42}]"#,
        ],
    )
    .success();
}

#[test]
fn proposals_surface_for_changed_evidence_and_explicit_territory() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    init_repo(&repo);
    create_requirement_topic_and_proposal(&repo);

    provenance(
        &repo,
        &[
            "proposals",
            "surface",
            "--scope",
            "default",
            "--changed-path",
            "src/payroll.rs",
            "--format",
            "json",
        ],
    )
    .success()
    .stdout(contains(r#""id": "proposal_overtime""#))
    .stdout(contains(r#""trigger": "evidence_site""#));

    provenance(
        &repo,
        &[
            "proposals",
            "surface",
            "--scope",
            "default",
            "--target-type",
            "requirement",
            "--target-id",
            "req_overtime",
            "--format",
            "json",
        ],
    )
    .success()
    .stdout(contains(r#""id": "proposal_overtime""#))
    .stdout(contains(r#""trigger": "territory""#));
}

/// `--target-type` and `--target-id` name one territory between them, so half
/// the family names nothing to surface against. The refusal reads back both
/// flags, which is the whole remedy the operator needs.
#[test]
#[verifies("rule_flag_sets_complete", examples)]
fn surfacing_against_half_a_target_family_names_both_flags() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    init_repo(&repo);
    create_requirement_topic_and_proposal(&repo);

    provenance(
        &repo,
        &[
            "proposals",
            "surface",
            "--scope",
            "default",
            "--target-type",
            "requirement",
            "--format",
            "json",
        ],
    )
    .failure()
    .stderr(contains(
        "--target-type and --target-id must be provided together",
    ));

    provenance(
        &repo,
        &[
            "proposals",
            "surface",
            "--scope",
            "default",
            "--target-id",
            "req_overtime",
            "--format",
            "json",
        ],
    )
    .failure()
    .stderr(contains(
        "--target-type and --target-id must be provided together",
    ));
}

#[test]
fn claiming_a_topic_returns_proposals_in_that_territory() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    init_repo(&repo);
    create_requirement_topic_and_proposal(&repo);

    let assert = provenance(
        &repo,
        &[
            "topics",
            "claim",
            "--scope",
            "default",
            "--id",
            "topic_overtime",
            "--actor",
            "agent-one",
            "--format",
            "json",
        ],
    )
    .success()
    .stdout(contains(r#""surfaced_proposals""#))
    .stdout(contains(r#""id": "proposal_overtime""#));
    let output: serde_json::Value = serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(output["id"], "topic_overtime");
    assert_eq!(output["claimed_by"], "agent-one");
}

#[test]
fn claiming_a_topic_carries_the_cli_changed_paths_into_proposal_demand() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    init_repo(&repo);
    create_requirement_topic_and_proposal(&repo);
    provenance(
        &repo,
        &[
            "proposals",
            "create",
            "--scope",
            "default",
            "--id",
            "proposal_evidence_only",
            "--proposal-key",
            "evidence-only",
            "--proposal-type",
            "requirement_candidate",
            "--title",
            "Check payroll evidence",
            "--summary",
            "Evidence outside the claimed territory",
            "--target-type",
            "requirement",
            "--target-id",
            "req_elsewhere",
            "--evidence-json",
            r#"[{"reference_id":"evidence_elsewhere","evidence_type":"artifact","summary":"implementation","file_path":"src/payroll.rs","line":7}]"#,
        ],
    )
    .success();

    provenance(
        &repo,
        &[
            "topics",
            "claim",
            "--scope",
            "default",
            "--id",
            "topic_overtime",
            "--actor",
            "agent-one",
            "--changed-path",
            "src/payroll.rs",
            "--format",
            "json",
        ],
    )
    .success()
    .stdout(contains(r#""id": "proposal_evidence_only""#))
    .stdout(contains(r#""trigger": "evidence_site""#));
}

#[test]
fn a_surface_read_failure_does_not_persist_the_topic_claim() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    init_repo(&repo);
    create_requirement_topic_and_proposal(&repo);
    std::fs::write(
        dir.path()
            .join("repo/.provenance/state/scopes/default/ideation/proposal_cards.jsonl"),
        "not-json\n",
    )
    .unwrap();

    provenance(
        &repo,
        &[
            "topics",
            "claim",
            "--scope",
            "default",
            "--id",
            "topic_overtime",
            "--actor",
            "agent-one",
            "--format",
            "json",
        ],
    )
    .failure();

    provenance(
        &repo,
        &["topics", "list", "--scope", "default", "--format", "json"],
    )
    .success()
    .stdout(predicates::str::contains("claimed_by").not());
}
