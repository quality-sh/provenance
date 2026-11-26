use super::bounded_process::{provenance, Bounded};
use super::support::{create_source, init_repo, write_run_dir};
use predicates::prelude::PredicateBooleanExt;

#[test]
fn swarm_backtrace_land_writes_run_dir_outputs_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let run_dir = dir.path().join("run");
    init_repo(&repo);
    create_source(&repo);
    write_run_dir(&run_dir, "Publishing is guarded by worker assignment.");
    let run_dir = run_dir.to_string_lossy().to_string();

    provenance()
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
        .bounded_assert()
        .success()
        .stdout(predicates::str::contains(r#""contributions": 2"#))
        .stdout(predicates::str::contains(r#""synthesis_packets": 1"#))
        .stdout(predicates::str::contains(r#""proposals": 1"#));

    super::export_fixture_support::make_default_scope_portable(&repo);
    provenance()
        .args([
            "export", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .bounded_assert()
        .success()
        .stdout(predicates::str::contains("contrib_backtrace_extract_auth"))
        .stdout(predicates::str::contains("synth_backtrace_auth"))
        .stdout(predicates::str::contains(
            "prop_req_publish_requires_worker",
        ))
        .stdout(predicates::str::contains(
            "assertion_req_publish_requires_worker",
        ))
        .stdout(predicates::str::contains(r#""confidence": 0.91"#));

    provenance()
        .args([
            "api",
            "proposals",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .bounded_assert()
        .success()
        .stdout(predicates::str::contains(
            r#""promotion_state": "asserted""#,
        ));
}

#[test]
fn swarm_backtrace_land_cannot_replace_asserted_run_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let run_dir = dir.path().join("run");
    init_repo(&repo);
    create_source(&repo);
    write_run_dir(&run_dir, "Original extracted finding.");
    let run_dir_arg = run_dir.to_string_lossy().to_string();

    provenance()
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
        .bounded_assert()
        .success();
    write_run_dir(&run_dir, "Updated extracted finding.");
    let merge_path = run_dir.join("merge/merged.json");
    let mut merge: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&merge_path).unwrap()).unwrap();
    merge["proposals"] = serde_json::json!([]);
    merge["assertions"] = serde_json::json!([]);
    std::fs::write(&merge_path, serde_json::to_vec(&merge).unwrap()).unwrap();
    provenance()
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
        .bounded_assert()
        .failure()
        .stderr(predicates::str::contains("referenced by an assertion"));

    super::export_fixture_support::make_default_scope_portable(&repo);
    provenance()
        .args([
            "export", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .bounded_assert()
        .success()
        .stdout(predicates::str::contains("Original extracted finding."))
        .stdout(predicates::str::contains("Updated extracted finding.").not())
        .stdout(predicates::str::contains(r#""contributions""#));
}

#[test]
fn identical_proposal_and_assertion_relanding_without_replace_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let run_dir = dir.path().join("run");
    init_repo(&repo);
    create_source(&repo);
    write_run_dir(&run_dir, "Original extracted finding.");
    let run_dir_arg = run_dir.to_string_lossy().to_string();

    let land = || {
        provenance()
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
            .bounded_assert()
    };
    land().success();
    std::fs::remove_dir_all(run_dir.join("extractors")).unwrap();
    std::fs::remove_dir_all(run_dir.join("refuters")).unwrap();
    let merge_path = run_dir.join("merge/merged.json");
    let mut merge: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&merge_path).unwrap()).unwrap();
    merge.as_object_mut().unwrap().remove("synthesis_packet");
    std::fs::write(&merge_path, serde_json::to_vec(&merge).unwrap()).unwrap();

    land()
        .failure()
        .stderr(predicates::str::contains("already exists"));

    super::export_fixture_support::make_default_scope_portable(&repo);
    let output = provenance()
        .args([
            "export", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .bounded_output();
    assert!(output.status.success());
    let export: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(export["proposal_cards"].as_array().unwrap().len(), 1);
    assert_eq!(export["assertion_records"].as_array().unwrap().len(), 1);
}

#[test]
fn swarm_backtrace_land_skips_scratch_files_beside_the_merge_output() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let run_dir = dir.path().join("run");
    init_repo(&repo);
    create_source(&repo);
    write_run_dir(&run_dir, "Publishing is guarded by worker assignment.");
    let merge: serde_json::Value =
        serde_json::from_slice(&std::fs::read(run_dir.join("merge/merged.json")).unwrap()).unwrap();
    std::fs::write(
        run_dir.join("merge/assertion.sample.json"),
        merge["assertions"][0].to_string(),
    )
    .unwrap();
    std::fs::write(run_dir.join("notes.json"), r#"{"scratch": true}"#).unwrap();
    let run_dir = run_dir.to_string_lossy().to_string();

    provenance()
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
        .bounded_assert()
        .success()
        .stdout(predicates::str::contains(r#""proposals": 1"#))
        .stdout(predicates::str::contains(r#""assertions": 1"#));
}
