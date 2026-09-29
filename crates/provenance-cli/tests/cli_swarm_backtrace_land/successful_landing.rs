use super::support::{create_source, init_repo, write_run_dir};
use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use std::io::Write;

#[test]
fn swarm_backtrace_land_writes_run_dir_outputs_end_to_end() {
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
        .success()
        .stdout(predicates::str::contains(r#""contributions": 2"#))
        .stdout(predicates::str::contains(r#""synthesis_packets": 1"#))
        .stdout(predicates::str::contains(r#""proposals": 1"#));

    crate::export_fixture_support::make_default_scope_portable(&repo);
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "export", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .assert()
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

#[test]
fn swarm_backtrace_land_cannot_replace_asserted_run_outputs() {
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
    let merge_path = run_dir.join("merge/merged.json");
    let mut merge: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&merge_path).unwrap()).unwrap();
    merge["proposals"] = serde_json::json!([]);
    merge["assertions"] = serde_json::json!([]);
    std::fs::write(&merge_path, serde_json::to_vec(&merge).unwrap()).unwrap();
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
        .stderr(predicates::str::contains("referenced by an assertion"));

    crate::export_fixture_support::make_default_scope_portable(&repo);
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "export", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .assert()
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

    crate::export_fixture_support::make_default_scope_portable(&repo);
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "export", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let export: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(export["proposal_cards"].as_array().unwrap().len(), 1);
    assert_eq!(export["assertion_records"].as_array().unwrap().len(), 1);
}

#[test]
fn check_rejects_landing_overlays_that_rewrite_asserted_evidence() {
    for evidence_kind in ["contribution", "synthesis"] {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo").to_string_lossy().to_string();
        let run_dir = dir.path().join("run");
        init_repo(&repo);
        create_source(&repo);
        write_run_dir(&run_dir, "Original extracted finding.");
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
            ])
            .assert()
            .success();

        let landings = std::path::Path::new(&repo)
            .join(".provenance/state/scopes/default/ideation/landings.jsonl");
        let first: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(&landings)
                .unwrap()
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap();
        let mut overlay = serde_json::json!({
            "contributions": [],
            "synthesis_packets": [],
            "proposals": [],
            "assertions": [],
            "dispositions": []
        });
        if evidence_kind == "contribution" {
            let mut contribution = first["contributions"][0].clone();
            contribution["strongest_finding"] = serde_json::json!("Forged finding.");
            overlay["contributions"] = serde_json::json!([contribution]);
        } else {
            let mut synthesis = first["synthesis_packets"][0].clone();
            synthesis["summary"] = serde_json::json!("Forged synthesis.");
            overlay["synthesis_packets"] = serde_json::json!([synthesis]);
        }
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(landings)
            .unwrap();
        writeln!(file, "{}", serde_json::to_string(&overlay).unwrap()).unwrap();

        Command::cargo_bin("provenance")
            .unwrap()
            .args(["check", "--repo", &repo, "--format", "json"])
            .assert()
            .failure()
            .stderr(predicates::str::contains("referenced by an assertion"));
    }
}
