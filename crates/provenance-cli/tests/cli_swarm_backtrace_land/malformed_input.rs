use super::support::{create_source, init_repo, write_run_dir};
use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use provenance_core::SUPPORTED_SCHEMA_VERSION;

fn write_bad_nested_stable_id(root: &std::path::Path) {
    let extractors = root.join("extractors");
    let merge = root.join("merge");
    std::fs::create_dir_all(&extractors).unwrap();
    std::fs::create_dir_all(&merge).unwrap();
    std::fs::write(
        extractors.join("bad.json"),
        serde_json::json!({
          "contribution": {
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": "default",
            "id": "contrib_backtrace_extract_auth",
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
          }
        }).to_string(),
    )
    .unwrap();
    std::fs::write(
        merge.join("merged.json"),
        serde_json::json!({
          "synthesis_packet": {
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": "default",
            "id": "synth_backtrace_auth",
            "target": {"artifact_type": "source", "artifact_id": "source_codebase"},
            "summary": "Extractor found a guard.",
            "consensus": [],
            "contested_claims": [],
            "minority_objections": [],
            "evidence_gaps": [],
            "unsupported_speculation": [],
            "open_questions": [],
            "suggested_artifacts": [],
            "required_human_decisions": []
          },
          "proposals": []
        })
        .to_string(),
    )
    .unwrap();
}

fn write_bad_proposal_confidence(root: &std::path::Path) {
    write_run_dir(root, "Publishing is guarded by worker assignment.");
    let merge_path = root.join("merge").join("merged.json");
    let merge_json = std::fs::read_to_string(&merge_path).unwrap();
    std::fs::write(
        &merge_path,
        merge_json.replace(r#""confidence":0.91"#, r#""confidence":1.5"#),
    )
    .unwrap();
}

fn write_merge_output_with_unknown_key(root: &std::path::Path) {
    write_run_dir(root, "Publishing is guarded by worker assignment.");
    let merge_path = root.join("merge").join("merged.json");
    let merge_json = std::fs::read_to_string(&merge_path).unwrap();
    std::fs::write(
        &merge_path,
        merge_json.replace(r#""proposals":["#, r#""proposal":["#),
    )
    .unwrap();
}

#[test]
fn swarm_backtrace_land_rejects_bad_proposal_confidence_before_writing() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let run_dir = dir.path().join("run");
    init_repo(&repo);
    create_source(&repo);
    write_bad_proposal_confidence(&run_dir);
    let run_dir = run_dir.to_string_lossy().to_string();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "swarm-backtrace",
            "land",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--run-dir",
            &run_dir,
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "confidence must be between 0.0 and 1.0",
        ));

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "export", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("contrib_backtrace_extract_auth").not())
        .stdout(predicates::str::contains("contrib_backtrace_refute_auth").not())
        .stdout(predicates::str::contains("synth_backtrace_auth").not())
        .stdout(predicates::str::contains("prop_req_publish_requires_worker").not());
}

#[test]
fn swarm_backtrace_land_rejects_unknown_merge_output_keys() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let run_dir = dir.path().join("run");
    init_repo(&repo);
    create_source(&repo);
    write_merge_output_with_unknown_key(&run_dir);
    let run_dir = run_dir.to_string_lossy().to_string();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "swarm-backtrace",
            "land",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--run-dir",
            &run_dir,
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown field `proposal`"));
}

#[test]
fn swarm_backtrace_land_reports_nested_stable_id_errors() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let run_dir = dir.path().join("run");
    init_repo(&repo);
    create_source(&repo);
    write_bad_nested_stable_id(&run_dir);
    let run_dir = run_dir.to_string_lossy().to_string();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "swarm-backtrace",
            "land",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--run-dir",
            &run_dir,
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("stable id"))
        .stderr(predicates::str::contains("evidence/auth"));
}
