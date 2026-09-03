use super::support::{export_scope, import_scope, init_repo, write_json};
use provenance_core::SUPPORTED_SCHEMA_VERSION;

#[test]
fn forged_terminal_import_fails_without_changing_live_scope() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init_repo(&repo, None);
    let baseline = dir.path().join("baseline.json");
    export_scope(&repo, &baseline).success();
    let before = std::fs::read(&baseline).unwrap();
    let mut forged: serde_json::Value = serde_json::from_slice(&before).unwrap();
    forged["proposal_cards"] = serde_json::json!([{
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "proposal_forged",
        "proposal_key": "forged", "proposal_type": "requirement_candidate",
        "title": "Forged", "summary": "Forged terminal ingress",
        "traceability": {
            "target": {"artifact_type": "requirement", "artifact_id": "req_missing"},
            "source_ids": [], "evidence_references": [], "supporting_claim_ids": []
        },
        "promotion_state": "accepted"
    }]);
    let input = dir.path().join("forged.json");
    write_json(&input, &forged);

    import_scope(&repo, &input)
        .failure()
        .stderr(predicates::str::contains("frozen shipped-v1 fingerprint"));

    let after = dir.path().join("after.json");
    export_scope(&repo, &after).success();
    assert_eq!(std::fs::read(after).unwrap(), before);
}

#[test]
fn late_scope_validation_failure_is_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init_repo(&repo, None);
    let baseline = dir.path().join("baseline.json");
    export_scope(&repo, &baseline).success();
    let before = std::fs::read(&baseline).unwrap();
    let mut invalid: serde_json::Value = serde_json::from_slice(&before).unwrap();
    invalid["rules"] = serde_json::json!([{
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "rule_invalid",
        "statement": "A rule naming a requirement that is not there.", "status": "active",
        "severity": "high", "requirement_ids": ["req_missing"]
    }]);
    let input = dir.path().join("invalid.json");
    write_json(&input, &invalid);

    import_scope(&repo, &input).failure();

    let after = dir.path().join("after.json");
    export_scope(&repo, &after).success();
    assert_eq!(std::fs::read(after).unwrap(), before);
    let transactions = repo.join(".provenance/cache/import-transactions");
    assert!(
        !transactions.exists() || std::fs::read_dir(transactions).unwrap().next().is_none(),
        "failed import must remove its staged transaction"
    );
}

#[test]
fn missing_disposition_canonical_artifact_import_is_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init_repo(&repo, Some("reviewer"));
    let baseline = dir.path().join("baseline.json");
    export_scope(&repo, &baseline).success();
    let before = std::fs::read(&baseline).unwrap();
    let mut invalid: serde_json::Value = serde_json::from_slice(&before).unwrap();
    invalid["sources"] = serde_json::json!([{
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "source_anchor",
        "name": "Anchor", "source_type": "document"
    }]);
    invalid["proposal_cards"] = serde_json::json!([{
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "proposal_a",
        "proposal_key": "a", "proposal_type": "source_gap", "title": "A", "summary": "A",
        "traceability": {"target": {"artifact_type": "source", "artifact_id": "source_anchor"},
            "source_ids": [], "evidence_references": [], "supporting_claim_ids": []},
        "promotion_state": "proposed"
    }]);
    invalid["dispositions"] = serde_json::json!([{
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "disposition_a",
        "proposal_id": "proposal_a", "decision": "rejected", "rationale": "Reviewed",
        "actor": {"identity_type": "human", "id": "reviewer"},
        "canonical_artifact": {"artifact_type": "requirement", "artifact_id": "req_missing"}
    }]);
    let input = dir.path().join("invalid-canonical-artifact.json");
    write_json(&input, &invalid);

    import_scope(&repo, &input)
        .failure()
        .stderr(predicates::str::contains(
            "canonical artifact does not exist",
        ));

    let after = dir.path().join("after.json");
    export_scope(&repo, &after).success();
    assert_eq!(std::fs::read(after).unwrap(), before);
}

#[test]
fn misfiled_disposition_canonical_artifact_import_is_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init_repo(&repo, Some("reviewer"));
    let baseline = dir.path().join("baseline.json");
    export_scope(&repo, &baseline).success();
    let before = std::fs::read(&baseline).unwrap();
    let mut invalid: serde_json::Value = serde_json::from_slice(&before).unwrap();
    invalid["requirements"] = serde_json::json!([{
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "other", "id": "req_misfiled",
        "statement": "Misfiled", "status": "active"
    }]);
    invalid["proposal_cards"] = serde_json::json!([{
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "proposal_a",
        "proposal_key": "a", "proposal_type": "requirement_candidate", "title": "A", "summary": "A",
        "traceability": {"target": {"artifact_type": "requirement", "artifact_id": "req_misfiled"},
            "source_ids": [], "evidence_references": [], "supporting_claim_ids": []},
        "promotion_state": "proposed"
    }]);
    invalid["dispositions"] = serde_json::json!([{
        "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "disposition_a",
        "proposal_id": "proposal_a", "decision": "rejected", "rationale": "Reviewed",
        "actor": {"identity_type": "human", "id": "reviewer"},
        "canonical_artifact": {"artifact_type": "requirement", "artifact_id": "req_misfiled"}
    }]);
    let input = dir.path().join("misfiled-canonical-artifact.json");
    write_json(&input, &invalid);

    import_scope(&repo, &input).failure();

    let after = dir.path().join("after.json");
    export_scope(&repo, &after).success();
    assert_eq!(std::fs::read(after).unwrap(), before);
}

#[test]
fn check_rejects_a_misfiled_disposition_target() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init_repo(&repo, Some("reviewer"));
    let scope = repo.join(".provenance/state/scopes/default");
    std::fs::create_dir_all(scope.join("requirements")).unwrap();
    std::fs::create_dir_all(scope.join("ideation")).unwrap();
    std::fs::write(
        scope.join("requirements/req.jsonl"),
        format!("{{\"schema_version\":{},\"scope_id\":\"other\",\"id\":\"req_misfiled\",\"statement\":\"Misfiled\",\"status\":\"active\"}}\n", SUPPORTED_SCHEMA_VERSION.0),
    )
    .unwrap();
    std::fs::write(
        scope.join("ideation/proposal_cards.jsonl"),
        format!("{{\"schema_version\":{},\"scope_id\":\"default\",\"id\":\"proposal_a\",\"proposal_key\":\"a\",\"proposal_type\":\"requirement_candidate\",\"title\":\"A\",\"summary\":\"A\",\"traceability\":{{\"target\":{{\"artifact_type\":\"requirement\",\"artifact_id\":\"req_misfiled\"}},\"source_ids\":[],\"evidence_references\":[],\"supporting_claim_ids\":[]}},\"promotion_state\":\"proposed\"}}\n", SUPPORTED_SCHEMA_VERSION.0),
    )
    .unwrap();
    std::fs::write(
        scope.join("ideation/dispositions.jsonl"),
        format!("{{\"schema_version\":{},\"scope_id\":\"default\",\"id\":\"disposition_a\",\"proposal_id\":\"proposal_a\",\"decision\":\"rejected\",\"rationale\":\"Reviewed\",\"actor\":{{\"identity_type\":\"human\",\"id\":\"reviewer\"}},\"canonical_artifact\":{{\"artifact_type\":\"requirement\",\"artifact_id\":\"req_misfiled\"}}}}\n", SUPPORTED_SCHEMA_VERSION.0),
    )
    .unwrap();

    super::support::provenance()
        .args(["check", "--repo", repo.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "canonical artifact does not exist",
        ));
}

#[test]
fn concurrent_first_imports_share_pristine_transaction_directory_setup() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init_repo(&repo, None);
    let input = dir.path().join("input.json");
    export_scope(&repo, &input).success();
    std::fs::remove_dir_all(repo.join(".provenance/cache")).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let imports = (0..8)
        .map(|_| {
            let barrier = barrier.clone();
            let repo = repo.clone();
            let input = input.clone();
            std::thread::spawn(move || {
                barrier.wait();
                std::process::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
                    .args([
                        "import",
                        "--repo",
                        repo.to_str().unwrap(),
                        "--scope",
                        "default",
                        "--input",
                        input.to_str().unwrap(),
                        "--dry-run",
                    ])
                    .output()
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();

    for import in imports {
        let output = import.join().unwrap();
        assert!(
            output.status.success(),
            "concurrent import failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(unix)]
#[test]
fn import_rejects_external_file_symlink_without_changing_live_state() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init_repo(&repo, None);
    let external = dir.path().join("secret");
    std::fs::write(&external, "do not import").unwrap();
    let link = repo.join(".provenance/state/external-link");
    std::os::unix::fs::symlink(&external, &link).unwrap();
    let input = dir.path().join("input.json");
    export_scope(&repo, &input).success();

    import_scope(&repo, &input)
        .failure()
        .stderr(predicates::str::contains("unsupported state entry"));

    let metadata = std::fs::symlink_metadata(&link).unwrap();
    assert!(metadata.file_type().is_symlink());
    assert_eq!(std::fs::read_to_string(&external).unwrap(), "do not import");
}

#[cfg(unix)]
#[test]
fn dry_run_rejects_symlinked_import_transactions_directory() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init_repo(&repo, None);
    let input = dir.path().join("input.json");
    export_scope(&repo, &input).success();
    let outside = dir.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let transactions = repo.join(".provenance/cache/import-transactions");
    if transactions.exists() {
        std::fs::remove_dir_all(&transactions).unwrap();
    }
    std::os::unix::fs::symlink(&outside, &transactions).unwrap();

    super::support::provenance()
        .args([
            "import",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--input",
            input.to_str().unwrap(),
            "--dry-run",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("symlink component"));

    let outside_entries = std::fs::read_dir(outside)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert!(
        outside_entries.is_empty(),
        "outside writes: {outside_entries:?}"
    );
}

#[cfg(unix)]
#[test]
fn dry_run_rejects_symlinked_cache_before_locking() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    init_repo(&repo, None);
    let input = dir.path().join("input.json");
    export_scope(&repo, &input).success();
    let outside = dir.path().join("outside-cache");
    std::fs::create_dir(&outside).unwrap();
    let cache = repo.join(".provenance/cache");
    std::fs::remove_dir_all(&cache).unwrap();
    std::os::unix::fs::symlink(&outside, &cache).unwrap();

    super::support::provenance()
        .args([
            "import",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--input",
            input.to_str().unwrap(),
            "--dry-run",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("symlink component"));

    let outside_entries = std::fs::read_dir(outside)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert!(
        outside_entries.is_empty(),
        "outside writes: {outside_entries:?}"
    );
}
