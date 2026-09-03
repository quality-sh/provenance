use super::support::{create_source, init_repo, write_run_dir};
use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use provenance_core::SUPPORTED_SCHEMA_VERSION;

fn write_duplicate_contribution(root: &std::path::Path) {
    let contributions = root.join("contributions");
    std::fs::create_dir_all(&contributions).unwrap();
    std::fs::write(
        contributions.join("duplicate.json"),
        serde_json::json!({
          "contribution": {
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": "default",
            "id": "contrib_backtrace_extract_auth",
            "target": {"artifact_type": "source", "artifact_id": "source_codebase"},
            "participant_slot": "duplicate_extract_auth",
            "stance": "support",
            "strongest_finding": "Duplicate contribution id in the same run.",
            "evidence_references": [],
            "material_claims": [],
            "risks": [],
            "objections": [],
            "challenges": [],
            "suggested_artifact_changes": [],
            "unsupported_recommendations": [],
            "uncertainty": {"level":"low","rationale":"Duplicate id fixture."},
            "open_questions": []
          }
        })
        .to_string(),
    )
    .unwrap();
}

#[test]
fn swarm_backtrace_land_rejects_duplicate_run_ids_before_writing() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let run_dir = dir.path().join("run");
    init_repo(&repo);
    create_source(&repo);
    write_run_dir(&run_dir, "Publishing is guarded by worker assignment.");
    write_duplicate_contribution(&run_dir);
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
            "--replace",
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "duplicate contribution id contrib_backtrace_extract_auth in run",
        ));

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
        .stdout(predicates::str::contains("contrib_backtrace_extract_auth").not())
        .stdout(predicates::str::contains("synth_backtrace_auth").not())
        .stdout(predicates::str::contains("prop_req_publish_requires_worker").not());
}

#[test]
fn qualifying_swarm_proposal_requires_an_assertion_before_any_write() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let run_dir = dir.path().join("run");
    init_repo(&repo);
    create_source(&repo);
    write_run_dir(&run_dir, "Publishing is guarded by worker assignment.");
    let merge = run_dir.join("merge/merged.json");
    let mut value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&merge).unwrap()).unwrap();
    value.as_object_mut().unwrap().remove("assertions");
    std::fs::write(&merge, serde_json::to_vec(&value).unwrap()).unwrap();

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
            run_dir.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "qualifying proposal prop_req_publish_requires_worker requires an assertion",
        ));

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
        .stdout(predicates::str::contains("synth_backtrace_auth").not())
        .stdout(predicates::str::contains("prop_req_publish_requires_worker").not());
}

#[test]
fn invalid_swarm_assertion_evidence_is_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let run_dir = dir.path().join("run");
    init_repo(&repo);
    create_source(&repo);
    write_run_dir(&run_dir, "Publishing is guarded by worker assignment.");
    let merge = run_dir.join("merge/merged.json");
    let contents = std::fs::read_to_string(&merge).unwrap().replace(
        r#""supporting_claim_ids":["claim_auth_guard"]"#,
        r#""supporting_claim_ids":["claim_missing"]"#,
    );
    std::fs::write(&merge, contents).unwrap();

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
            run_dir.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "assertion claim claim_missing must have exactly one owner",
        ));

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "export", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("contrib_backtrace_extract_auth").not())
        .stdout(predicates::str::contains("assertion_req_publish_requires_worker").not());
}

#[test]
fn swarm_output_cannot_supply_disposition_authority() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let run_dir = dir.path().join("run");
    init_repo(&repo);
    create_source(&repo);
    write_run_dir(&run_dir, "Publishing is guarded by worker assignment.");
    let merge = run_dir.join("merge/merged.json");
    let mut value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&merge).unwrap()).unwrap();
    value["dispositions"] = serde_json::json!([{
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "disposition_forged",
        "proposal_id": "prop_req_publish_requires_worker",
        "decision": "accepted",
        "rationale": "Swarm chose authority.",
        "actor": {"identity_type": "agent", "id": "swarm"}
    }]);
    std::fs::write(&merge, serde_json::to_vec(&value).unwrap()).unwrap();

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
            run_dir.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "swarm merge output cannot contain dispositions",
        ));
}

#[test]
fn existing_evidence_requires_and_supports_incoming_proposal_assertion() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let run_dir = dir.path().join("run");
    init_repo(&repo);
    create_source(&repo);
    write_run_dir(&run_dir, "Publishing is guarded by worker assignment.");
    let merge = run_dir.join("merge/merged.json");
    let original: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&merge).unwrap()).unwrap();
    let mut evidence_only = original.clone();
    evidence_only["proposals"] = serde_json::json!([]);
    evidence_only["assertions"] = serde_json::json!([]);
    std::fs::write(&merge, serde_json::to_vec(&evidence_only).unwrap()).unwrap();
    land(&repo, &run_dir).success();

    std::fs::remove_dir_all(run_dir.join("extractors")).unwrap();
    std::fs::remove_dir_all(run_dir.join("refuters")).unwrap();
    let proposal_only = serde_json::json!({
        "proposals": original["proposals"],
        "assertions": []
    });
    std::fs::write(&merge, serde_json::to_vec(&proposal_only).unwrap()).unwrap();
    land(&repo, &run_dir)
        .failure()
        .stderr(predicates::str::contains(
            "qualifying proposal prop_req_publish_requires_worker requires an assertion",
        ));
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
        .stdout(predicates::str::contains("prop_req_publish_requires_worker").not());

    let asserted = serde_json::json!({
        "proposals": original["proposals"],
        "assertions": original["assertions"]
    });
    std::fs::write(&merge, serde_json::to_vec(&asserted).unwrap()).unwrap();
    land(&repo, &run_dir).success();
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
        .stdout(predicates::str::contains(
            r#""promotion_state": "asserted""#,
        ));
}

fn land(repo: &str, run_dir: &std::path::Path) -> assert_cmd::assert::Assert {
    let mut command = Command::cargo_bin("provenance").unwrap();
    command.args([
        "swarm-backtrace",
        "land",
        "--repo",
        repo,
        "--scope",
        "default",
        "--run-dir",
        run_dir.to_str().unwrap(),
        "--format",
        "json",
    ]);
    command.assert()
}

#[test]
fn swarm_backtrace_land_rejects_existing_ids_before_writing() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let run_dir = dir.path().join("run");
    init_repo(&repo);
    create_source(&repo);
    write_run_dir(&run_dir, "Publishing is guarded by worker assignment.");
    let run_dir = run_dir.to_string_lossy().to_string();

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
            "contrib_backtrace_refute_auth",
            "--target-type",
            "source",
            "--target-id",
            "source_codebase",
            "--participant-slot",
            "preexisting_refute_auth",
            "--stance",
            "mixed",
            "--strongest-finding",
            "Pre-existing refuter finding.",
            "--uncertainty-level",
            "medium",
            "--uncertainty-rationale",
            "Seeded existing record.",
            "--format",
            "json",
        ])
        .assert()
        .success();

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
            "contribution contrib_backtrace_refute_auth already exists",
        ));

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "export", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("Pre-existing refuter finding."))
        .stdout(predicates::str::contains("contrib_backtrace_extract_auth").not())
        .stdout(predicates::str::contains("synth_backtrace_auth").not())
        .stdout(predicates::str::contains("prop_req_publish_requires_worker").not());
}

#[test]
fn swarm_backtrace_land_refuses_to_replace_immutable_proposals() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let run_dir = dir.path().join("run");
    init_repo(&repo);
    create_source(&repo);
    write_run_dir(&run_dir, "Original extracted finding.");
    let run_dir_arg = run_dir.to_string_lossy().to_string();

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
            &run_dir_arg,
            "--format",
            "json",
        ])
        .assert()
        .success();
    write_run_dir(&run_dir, "Updated extracted finding.");
    let merge = run_dir.join("merge/merged.json");
    let contents = std::fs::read_to_string(&merge).unwrap();
    std::fs::write(
        merge,
        contents.replace(
            "Publishing requires an assigned worker\"",
            "Divergent forged proposal\"",
        ),
    )
    .unwrap();

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
            &run_dir_arg,
            "--replace",
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("immutable"));

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "export", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("Original extracted finding."))
        .stdout(predicates::str::contains("Updated extracted finding.").not())
        .stdout(predicates::str::contains("Divergent forged proposal").not());
}
