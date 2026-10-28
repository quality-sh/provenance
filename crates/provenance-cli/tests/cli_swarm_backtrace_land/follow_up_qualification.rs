use super::support::{create_source, init_repo, write_run_dir};
use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

#[test]
fn incoming_synthesis_requires_and_supports_asserting_an_existing_proposal() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();
    let run_dir = dir.path().join("run");
    init_repo(&repo);
    create_source(&repo);
    write_run_dir(&run_dir, "Publishing is guarded by worker assignment.");
    let merge = run_dir.join("merge/merged.json");
    let original: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&merge).unwrap()).unwrap();

    std::fs::remove_dir_all(run_dir.join("extractors")).unwrap();
    std::fs::remove_dir_all(run_dir.join("refuters")).unwrap();
    std::fs::write(
        &merge,
        serde_json::to_vec(&serde_json::json!({
            "proposals": original["proposals"],
            "assertions": []
        }))
        .unwrap(),
    )
    .unwrap();
    land(&repo, &run_dir).success();

    write_run_dir(&run_dir, "Publishing is guarded by worker assignment.");
    let evidence_only = serde_json::json!({
        "synthesis_packet": original["synthesis_packet"],
        "proposals": [],
        "assertions": []
    });
    std::fs::write(&merge, serde_json::to_vec(&evidence_only).unwrap()).unwrap();
    land(&repo, &run_dir)
        .failure()
        .stderr(predicates::str::contains(
            "qualifying proposal prop_req_publish_requires_worker requires an assertion",
        ));
    crate::export_fixture_support::make_default_scope_portable(&repo);
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "export", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("synth_backtrace_auth").not())
        .stdout(predicates::str::contains("contrib_backtrace_extract_auth").not());

    let asserted = serde_json::json!({
        "synthesis_packet": original["synthesis_packet"],
        "proposals": [],
        "assertions": original["assertions"]
    });
    std::fs::write(&merge, serde_json::to_vec(&asserted).unwrap()).unwrap();
    land(&repo, &run_dir)
        .success()
        .stdout(predicates::str::contains(r#""assertions": 1"#));
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
