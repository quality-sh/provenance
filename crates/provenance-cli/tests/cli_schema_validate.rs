use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_macros::verifies;
use provenance_store::graph_reference::{graph_digest, GraphExport};
use serde_json::{json, Value};

fn write_json(dir: &tempfile::TempDir, name: &str, json: impl AsRef<str>) -> String {
    let path = dir.path().join(name);
    std::fs::write(&path, json.as_ref()).unwrap();
    path.to_string_lossy().to_string()
}

/// A pinned graph carrying one source and nothing else.
fn graph_with_source(source: &Value) -> Value {
    json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope": {"id": "default", "path_prefix": "."},
        "sources": [source],
        "domains": [],
        "requirements": [],
        "boundaries": [],
        "topics": [],
        "questions": [],
        "resolutions": [],
        "rules": []
    })
}

/// An exact-export document carrying `graph` and the digest that graph hashes
/// to. The digest is derived here rather than written down, so no fixture
/// claims a hash its graph does not have and each refusal below is earned by
/// the field its test set out to break.
fn export_document(graph: &Value) -> Value {
    let parsed: GraphExport =
        serde_json::from_value(graph.clone()).expect("the fixture graph is a pinned graph");
    json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "operation": "exact-export",
        "reference_id": format!("grf1_{}", "0".repeat(64)),
        "graph_digest": graph_digest(&parsed).expect("a graph can be canonicalized"),
        "graph": graph
    })
}

#[test]
fn schema_show_outputs_ideation_artifact_schemas() {
    for (artifact, expected_title) in [
        ("contribution", "Contribution"),
        ("synthesis-packet", "SynthesisPacket"),
        ("proposal", "ProposalCard"),
    ] {
        Command::cargo_bin("provenance")
            .unwrap()
            .args(["schema", "show", artifact, "--format", "json"])
            .assert()
            .success()
            .stdout(predicates::str::contains(expected_title))
            .stdout(predicates::str::contains("stableId"))
            .stdout(predicates::str::contains("^[a-z0-9_-]+$"));
    }
}

#[test]
fn disposition_schema_closes_canonical_artifact_and_validation_rejects_unknown_fields() {
    let schema = Command::cargo_bin("provenance")
        .unwrap()
        .args(["schema", "show", "disposition", "--format", "json"])
        .output()
        .unwrap();
    assert!(schema.status.success());
    let schema: serde_json::Value = serde_json::from_slice(&schema.stdout).unwrap();
    let canonical = &schema["schema"]["properties"]["canonical_artifact"];
    assert_eq!(canonical["additionalProperties"], false);
    assert_eq!(
        canonical["required"],
        serde_json::json!(["artifact_type", "artifact_id"])
    );
    let external_action = &schema["schema"]["properties"]["external_action"];
    assert_eq!(external_action["additionalProperties"], false);
    assert_eq!(
        external_action["required"],
        serde_json::json!(["system", "scope", "kind", "key"])
    );

    let dir = tempfile::tempdir().unwrap();
    let invalid = write_json(
        &dir,
        "invalid-disposition.json",
        serde_json::json!({
          "schema_version": SUPPORTED_SCHEMA_VERSION.0,
          "scope_id": "default",
          "id": "disposition_accept",
          "proposal_id": "proposal_candidate",
          "decision": "accepted",
          "rationale": "Accepted.",
          "actor": {"identity_type": "human", "id": "reviewer"},
          "canonical_artifact": {
            "artifact_type": "requirement",
            "artifact_id": "requirement_one",
            "unexpected": true
          }
        })
        .to_string(),
    );
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "validate",
            "disposition",
            "--input",
            &invalid,
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown field `unexpected`"));

    assert_external_action_unknown_field_rejected(&dir);

    let empty = write_json(
        &dir,
        "empty-disposition.json",
        serde_json::json!({
          "schema_version": SUPPORTED_SCHEMA_VERSION.0,
          "scope_id": "default",
          "id": "disposition_accept",
          "proposal_id": "proposal_candidate",
          "decision": "accepted",
          "rationale": "",
          "actor": {"identity_type": "human", "id": ""}
        })
        .to_string(),
    );
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "validate",
            "disposition",
            "--input",
            &empty,
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "disposition rationale must not be empty",
        ));
}

fn assert_external_action_unknown_field_rejected(dir: &tempfile::TempDir) {
    let unknown_action = write_json(
        dir,
        "unknown-action-field.json",
        serde_json::json!({
          "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "disposition_accept",
          "proposal_id": "proposal_candidate", "decision": "rejected", "rationale": "Rejected.",
          "actor": {"identity_type": "human", "id": "reviewer"},
          "external_action": {"system": "github", "scope": "acme/payroll", "kind": "issue", "key": "44", "workflow_state": "closed"}
        }).to_string(),
    );
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["validate", "disposition", "--input", &unknown_action])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown field `workflow_state`"));
}

#[test]
fn assertion_validation_matches_closed_schema_invariants() {
    let dir = tempfile::tempdir().unwrap();
    let base = json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "assertion_candidate",
        "proposal_id": "proposal_candidate",
        "synthesis_packet_id": "synthesis_candidate",
        "supporting_claim_ids": ["claim_one"]
    });
    for (name, mutate, expected) in [
        ("empty", "empty", "requires positive evidence"),
        ("duplicate", "duplicate", "is repeated"),
        ("unknown", "unknown", "unknown field `unexpected`"),
    ] {
        let mut value = base.clone();
        match mutate {
            "empty" => value["supporting_claim_ids"] = json!([]),
            "duplicate" => value["supporting_claim_ids"] = json!(["claim_one", "claim_one"]),
            "unknown" => value["unexpected"] = json!(true),
            _ => unreachable!(),
        }
        let input = write_json(
            &dir,
            &format!("{name}-assertion.json"),
            serde_json::to_string(&value).unwrap(),
        );
        Command::cargo_bin("provenance")
            .unwrap()
            .args(["validate", "assertion", "--input", &input])
            .assert()
            .failure()
            .stderr(predicates::str::contains(expected));
    }
}

#[test]
fn proposal_validation_rejects_unknown_nested_fields() {
    let dir = tempfile::tempdir().unwrap();
    let input = write_json(
        &dir,
        "unknown-proposal.json",
        serde_json::json!({
          "schema_version": SUPPORTED_SCHEMA_VERSION.0,
          "scope_id": "default",
          "id": "proposal_candidate",
          "proposal_key": "candidate",
          "proposal_type": "requirement_candidate",
          "title": "Candidate",
          "summary": "Candidate.",
          "traceability": {
            "target": {"artifact_type": "source", "artifact_id": "source_one", "unexpected": true},
            "source_ids": [],
            "evidence_references": [],
            "supporting_claim_ids": []
          },
          "promotion_state": "proposed"
        })
        .to_string(),
    );
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["validate", "proposal", "--input", &input])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown field `unexpected`"));
}

#[test]
fn validate_accepts_good_ideation_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    let contribution = write_json(
        &dir,
        "contribution.json",
        serde_json::json!({
          "schema_version": SUPPORTED_SCHEMA_VERSION.0,
          "scope_id": "default",
          "id": "contrib_extract_auth",
          "target": {"artifact_type": "source", "artifact_id": "source_codebase"},
          "participant_slot": "extract_auth",
          "stance": "support",
          "strongest_finding": "Publishing is guarded by worker assignment.",
          "evidence_references": [{"reference_id":"evidence_auth_guard","evidence_type":"artifact","summary":"Guard rejects missing worker","file_path":"src/auth.rs","line":12}],
          "material_claims": [{"claim_id":"claim_auth_guard","statement":"Publishing requires an assigned worker.","evidence_type":"artifact","evidence_reference_ids":["evidence_auth_guard"],"confidence":0.91}],
          "risks": [],
          "objections": [],
          "challenges": [],
          "suggested_artifact_changes": [],
          "unsupported_recommendations": [],
          "uncertainty": {"level":"low","rationale":"Direct guard evidence."},
          "open_questions": []
        }).to_string(),
    );
    let synthesis = write_json(
        &dir,
        "synthesis.json",
        serde_json::json!({
          "schema_version": SUPPORTED_SCHEMA_VERSION.0,
          "scope_id": "default",
          "id": "synth_backtrace_auth",
          "target": {"artifact_type": "source", "artifact_id": "source_codebase"},
          "summary": "Extractor and refuter agree on the guard.",
          "consensus": [{"statement":"Publishing is guarded.","supporting_participant_slots":["extract_auth"],"evidence_reference_ids":["evidence_auth_guard"]}],
          "contested_claims": [],
          "minority_objections": [],
          "evidence_gaps": [],
          "unsupported_speculation": [],
          "open_questions": [],
          "suggested_artifacts": [{"proposal_key":"backtrace/auth/publish_requires_worker","proposal_type":"requirement_candidate","summary":"Review the candidate requirement.","origin_participant_slots":["extract_auth"]}],
          "required_human_decisions": [{"decision_key":"decide_publish_guard","prompt":"Confirm this behavior is intentional.","blocks_promotion":true}]
        }).to_string(),
    );
    let proposal = write_json(
        &dir,
        "proposal.json",
        serde_json::json!({
          "schema_version": SUPPORTED_SCHEMA_VERSION.0,
          "scope_id": "default",
          "id": "prop_req_publish_requires_worker",
          "proposal_key": "backtrace/auth/publish_requires_worker",
          "proposal_type": "requirement_candidate",
          "title": "Publishing requires an assigned worker",
          "summary": "Candidate requirement extracted from the publishing guard.",
          "confidence": 0.91,
          "traceability": {
            "target": {"artifact_type": "source", "artifact_id": "source_codebase"},
            "source_ids": ["source_codebase"],
            "evidence_references": [{"reference_id":"evidence_auth_guard","evidence_type":"artifact","summary":"Guard rejects missing worker","file_path":"src/auth.rs","line":12}],
            "supporting_claim_ids": ["claim_auth_guard"]
          },
          "promotion_state": "proposed"
        }).to_string(),
    );

    for (artifact, path) in [
        ("contribution", contribution),
        ("synthesis-packet", synthesis),
        ("proposal", proposal),
    ] {
        Command::cargo_bin("provenance")
            .unwrap()
            .args(["validate", artifact, "--input", &path, "--format", "json"])
            .assert()
            .success()
            .stdout(predicates::str::contains(r#""valid": true"#))
            .stdout(predicates::str::contains(artifact));
    }
}

#[test]
fn validate_rejects_nested_invalid_stable_ids() {
    let dir = tempfile::tempdir().unwrap();
    let contribution = write_json(
        &dir,
        "bad-contribution.json",
        serde_json::json!({
          "schema_version": SUPPORTED_SCHEMA_VERSION.0,
          "scope_id": "default",
          "id": "contrib_extract_auth",
          "target": {"artifact_type": "source", "artifact_id": "source_codebase"},
          "participant_slot": "extract_auth",
          "stance": "support",
          "strongest_finding": "Publishing is guarded by worker assignment.",
          "evidence_references": [{"reference_id":"evidence/auth","evidence_type":"artifact","summary":"Bad nested id"}],
          "material_claims": [],
          "risks": [],
          "objections": [],
          "challenges": [],
          "suggested_artifact_changes": [],
          "unsupported_recommendations": [],
          "uncertainty": {"level":"low","rationale":"Direct guard evidence."},
          "open_questions": []
        }).to_string(),
    );
    let synthesis = write_json(
        &dir,
        "bad-synthesis.json",
        serde_json::json!({
          "schema_version": SUPPORTED_SCHEMA_VERSION.0,
          "scope_id": "default",
          "id": "synth_backtrace_auth",
          "target": {"artifact_type": "source", "artifact_id": "source_codebase"},
          "summary": "Extractor and refuter agree on the guard.",
          "consensus": [],
          "contested_claims": [],
          "minority_objections": [],
          "evidence_gaps": [],
          "unsupported_speculation": [],
          "open_questions": [],
          "suggested_artifacts": [],
          "required_human_decisions": [{"decision_key":"decide/publish_guard","prompt":"Confirm this behavior is intentional.","blocks_promotion":true}]
        }).to_string(),
    );
    let proposal = write_json(
        &dir,
        "bad-proposal.json",
        serde_json::json!({
          "schema_version": SUPPORTED_SCHEMA_VERSION.0,
          "scope_id": "default",
          "id": "prop_req_publish_requires_worker",
          "proposal_key": "backtrace/auth/publish_requires_worker",
          "proposal_type": "requirement_candidate",
          "title": "Publishing requires an assigned worker",
          "summary": "Candidate requirement extracted from the publishing guard.",
          "traceability": {
            "target": {"artifact_type": "source", "artifact_id": "source/codebase"},
            "source_ids": ["source/codebase"],
            "evidence_references": [],
            "supporting_claim_ids": []
          },
          "promotion_state": "proposed"
        })
        .to_string(),
    );

    for (artifact, path) in [
        ("contribution", contribution),
        ("synthesis-packet", synthesis),
        ("proposal", proposal),
    ] {
        Command::cargo_bin("provenance")
            .unwrap()
            .args(["validate", artifact, "--input", &path, "--format", "json"])
            .assert()
            .failure()
            .stderr(predicates::str::contains("stable id"));
    }
}

#[test]
fn validate_rejects_forbidden_graph_reference_export_fields() {
    let dir = tempfile::tempdir().unwrap();
    let export = export_document(&graph_with_source(&json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "source_policy",
        "name": "Policy",
        "source_type": "policy",
        "url": null
    })));

    for (name, field, value) in [
        ("collaboration", "origin_thread", json!("thread_private")),
        ("unknown", "unexpected", json!(true)),
    ] {
        let mut malformed = export.clone();
        malformed["graph"]["sources"][0][field] = value;
        let path = write_json(
            &dir,
            &format!("{name}-export.json"),
            serde_json::to_string(&malformed).unwrap(),
        );

        Command::cargo_bin("provenance")
            .unwrap()
            .args([
                "validate",
                "graph-reference-export",
                "--input",
                &path,
                "--format",
                "json",
            ])
            .assert()
            .failure();
    }
}

#[test]
#[verifies("rule_pinned_scope_ownership", examples)]
fn validate_rejects_graph_reference_export_records_from_another_scope() {
    let dir = tempfile::tempdir().unwrap();
    let export = export_document(&graph_with_source(&json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "other",
        "id": "source_policy",
        "name": "Policy",
        "source_type": "policy",
        "url": null
    })));
    let path = write_json(
        &dir,
        "cross-scope-export.json",
        serde_json::to_string(&export).unwrap(),
    );

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "validate",
            "graph-reference-export",
            "--input",
            &path,
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "source 'source_policy' belongs to scope 'other', not 'default'",
        ));
}
