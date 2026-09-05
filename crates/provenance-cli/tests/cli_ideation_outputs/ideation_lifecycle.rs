use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;

#[test]
fn dispositions_list_rejects_invalid_lifecycle_records() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
        ])
        .assert()
        .success();
    let dispositions = repo.join(".provenance/state/scopes/default/ideation/dispositions.jsonl");
    std::fs::create_dir_all(dispositions.parent().unwrap()).unwrap();
    std::fs::write(
        dispositions,
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"forged_disposition","proposal_id":"missing_proposal","decision":"accepted","rationale":"Forged","actor":{"identity_type":"human","id":"forger"}}).to_string() + "\n",
    )
    .unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "dispositions",
            "list",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("proposal does not exist"));
}

#[test]
#[allow(clippy::too_many_lines)]
fn cli_creates_materializes_and_exports_ideation_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_string_lossy().to_string();
    let export_path = dir.path().join("export.json").to_string_lossy().to_string();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            &repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
            "--disposition-actor-id",
            "ben",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "sources",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--id",
            "source_schads",
            "--name",
            "SCHADS Award",
            "--source-type",
            "policy",
            "--format",
            "json",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "requirements",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--id",
            "req_overtime",
            "--statement",
            "Overtime must be traceable",
            "--format",
            "json",
        ])
        .assert()
        .success();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "contributions",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--id",
            "contrib_reviewer_001",
            "--target-type",
            "requirement",
            "--target-id",
            "req_overtime",
            "--participant-slot",
            "reviewer",
            "--stance",
            "support",
            "--strongest-finding",
            "The requirement is supported by source and code evidence.",
            "--evidence-json",
            r#"[{"reference_id":"evidence_code_line","evidence_type":"artifact","summary":"Existing payroll check","file_path":"src/payroll/overtime.rs","line":42}]"#,
            "--claims-json",
            r#"[{"claim_id":"claim_overtime_threshold","statement":"Overtime starts after the award threshold.","evidence_type":"artifact","evidence_reference_ids":["evidence_code_line"],"confidence":0.87}]"#,
            "--risks-json",
            r#"["Payroll underpayment if the threshold is wrong"]"#,
            "--uncertainty-level",
            "medium",
            "--uncertainty-rationale",
            "Agreement overrides were not reviewed.",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("contrib_reviewer_001"))
        .stdout(predicates::str::contains(r#""confidence": 0.87"#));

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "synthesis-packets",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--id",
            "synth_overtime_001",
            "--target-type",
            "requirement",
            "--target-id",
            "req_overtime",
            "--summary",
            "Participants agree the threshold needs explicit traceability.",
            "--consensus-json",
            r#"[{"statement":"The requirement needs a source reference.","supporting_participant_slots":["reviewer"],"evidence_reference_ids":["evidence_code_line"]}]"#,
            "--evidence-gaps-json",
            r#"[{"question":"Which agreement applies?","needed_evidence_type":"source","blocking_promotion":false}]"#,
            "--suggested-artifacts-json",
            r#"[{"proposal_id":"proposal_overtime_traceability","proposal_key":"req-overtime-traceability","proposal_type":"requirement_candidate","summary":"Clarify source traceability.","origin_participant_slots":["reviewer"]}]"#,
            "--required-human-decisions-json",
            r#"[{"decision_key":"decide_agreement_scope","prompt":"Confirm the governing agreement.","blocks_promotion":true}]"#,
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("synth_overtime_001"));

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "proposals",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--id",
            "proposal_overtime_traceability",
            "--proposal-key",
            "req-overtime-traceability",
            "--proposal-type",
            "requirement_candidate",
            "--title",
            "Clarify overtime traceability",
            "--summary",
            "Add source-backed threshold language.",
            "--confidence",
            "0.83",
            "--target-type",
            "requirement",
            "--target-id",
            "req_overtime",
            "--source-id",
            "source_schads",
            "--evidence-json",
            r#"[{"reference_id":"evidence_code_line","evidence_type":"artifact","summary":"Existing payroll check","file_path":"src/payroll/overtime.rs","line":42}]"#,
            "--supporting-claim-id",
            "claim_overtime_threshold",
            "--format",
            "json",
        ])
        .assert()
        .success();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "proposals",
            "assert",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--id",
            "assertion_overtime_traceability",
            "--proposal-id",
            "proposal_overtime_traceability",
            "--synthesis-packet-id",
            "synth_overtime_001",
            "--supporting-claim-id",
            "claim_overtime_threshold",
            "--resolve-human-gate",
            "--decision-key",
            "decide_agreement_scope",
            "--format",
            "json",
        ])
        .assert()
        .success();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "dispositions",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--id",
            "decision_overtime_traceability",
            "--proposal-id",
            "proposal_overtime_traceability",
            "--decision",
            "accepted",
            "--rationale",
            "Human confirmed the source traceability.",
            "--actor-id",
            "ben",
            "--actor-type",
            "human",
            "--actor-name",
            "Ben",
            "--canonical-artifact-type",
            "requirement",
            "--canonical-artifact-id",
            "req_overtime",
            "--external-system",
            "github",
            "--external-scope",
            "acme/payroll",
            "--external-kind",
            "commit",
            "--external-key",
            "abc123",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("decision_overtime_traceability"));

    Command::cargo_bin("provenance")
        .unwrap()
        .args(["materialize", "--repo", &repo, "--format", "json"])
        .assert()
        .success()
        .stdout(predicates::str::contains(r#""records_loaded": 7"#));

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "proposals",
            "list",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("src/payroll/overtime.rs"))
        .stdout(predicates::str::contains(r#""confidence": 0.83"#))
        .stdout(predicates::str::contains(
            r#""promotion_state": "accepted""#,
        ));

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "export",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--format",
            "json",
            "--output",
            &export_path,
        ])
        .assert()
        .success();

    let exported = std::fs::read_to_string(export_path).unwrap();
    assert!(exported.contains(r#""proposal_cards""#));
    assert!(exported.contains(r#""confidence": 0.87"#));
    assert!(exported.contains(r#""confidence": 0.83"#));
    assert!(exported.contains(r#""dispositions""#));
    let exported_json: serde_json::Value = serde_json::from_str(&exported).unwrap();
    assert_eq!(
        exported_json["dispositions"][0]["external_action"],
        serde_json::json!({
            "system": "github", "scope": "acme/payroll", "kind": "commit", "key": "abc123"
        })
    );
    assert!(!exported.contains(r#""promotion_decisions""#));
    assert!(std::path::Path::new(&repo)
        .join(".provenance/state/scopes/default/ideation/dispositions.jsonl")
        .is_file());
    assert!(exported.contains(r#""contributions""#));
    assert!(exported.contains(r#""synthesis_packets""#));
    assert!(exported.contains(r#""assertion_records""#));
    assert!(exported.contains(r#""blocking_promotion": false"#));
}
