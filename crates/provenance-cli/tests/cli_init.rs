use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_macros::verifies;
use serde_json::Value;

fn init(repo: &std::path::Path, args: &[&str]) -> assert_cmd::assert::Assert {
    let mut command = Command::cargo_bin("provenance").unwrap();
    command.args(["init", "--path", repo.to_str().unwrap()]);
    command.args(args).assert()
}

fn read_manifest(repo: &std::path::Path) -> Value {
    serde_json::from_slice(&std::fs::read(repo.join(".provenance/state/manifest.json")).unwrap())
        .unwrap()
}

fn write_manifest(repo: &std::path::Path, manifest: &Value) {
    std::fs::write(
        repo.join(".provenance/state/manifest.json"),
        serde_json::to_vec_pretty(manifest).unwrap(),
    )
    .unwrap();
}

#[test]
fn cli_init_check_and_materialize_empty_repo() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();

    assert!(repo.join(".provenance/state/manifest.json").exists());
    assert!(!repo.join(".provenance/cache").exists());
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(repo.join(".provenance/state/manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["disposition_actor_ids"], serde_json::json!([]));

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "check",
            "--repo",
            repo.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "materialize",
            "--repo",
            repo.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    assert!(repo.join(".provenance/cache/provenance.db").exists());
}

#[test]
fn fresh_init_without_scope_fails_before_writing() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");

    init(&repo, &[]).failure();

    assert!(!repo.exists());
}

#[test]
fn init_rerun_without_manifest_flags_preserves_every_manifest_field() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    init(
        &repo,
        &[
            "--scope",
            "default",
            "--path-prefix",
            "src",
            "--disposition-actor-id",
            "reviewer",
        ],
    )
    .success();
    let mut original = read_manifest(&repo);
    original["scopes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id": "docs", "path_prefix": "docs"}));
    write_manifest(&repo, &original);

    init(&repo, &[]).success();

    assert_eq!(read_manifest(&repo), original);
}

/// A future manifest is refused outright, not preserved: the guard-all-reads
/// ruling covers init like every other read, so re-init never rewrites a
/// manifest this build cannot understand.
#[test]
fn init_rerun_refuses_a_future_manifest_version() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    init(&repo, &["--scope", "default"]).success();
    let mut original = read_manifest(&repo);
    original["schema_version"] = serde_json::json!(SUPPORTED_SCHEMA_VERSION.0 + 1);
    write_manifest(&repo, &original);

    init(&repo, &[])
        .failure()
        .stderr(predicates::str::contains(format!(
            "schema_version must be {}",
            SUPPORTED_SCHEMA_VERSION.0
        )));

    assert_eq!(read_manifest(&repo), original);
}

#[test]
fn init_rerun_with_actor_flag_updates_only_the_allowlist() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    init(
        &repo,
        &[
            "--scope",
            "default",
            "--path-prefix",
            "src",
            "--disposition-actor-id",
            "reviewer",
        ],
    )
    .success();
    let original = read_manifest(&repo);

    init(
        &repo,
        &[
            "--disposition-actor-id",
            "maintainer",
            "--disposition-actor-id",
            "release-manager",
        ],
    )
    .success();

    let mut expected = original;
    expected["disposition_actor_ids"] = serde_json::json!(["maintainer", "release-manager"]);
    assert_eq!(read_manifest(&repo), expected);
}

#[test]
fn init_clear_disposition_actors_only_empties_the_allowlist() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    init(
        &repo,
        &[
            "--scope",
            "default",
            "--path-prefix",
            "src",
            "--disposition-actor-id",
            "reviewer",
        ],
    )
    .success();
    let original = read_manifest(&repo);

    init(&repo, &["--clear-disposition-actors"]).success();

    let mut expected = original;
    expected["disposition_actor_ids"] = serde_json::json!([]);
    assert_eq!(read_manifest(&repo), expected);
}

#[test]
#[verifies("rule_init_plan_rejection_preserves_targets", examples)]
#[verifies("rule_init_validates_planned_repository", examples)]
fn planned_actor_change_is_rejected_without_managed_file_writes() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    init(
        &repo,
        &["--scope", "default", "--disposition-actor-id", "reviewer"],
    )
    .success();
    let ideation = repo.join(".provenance/state/scopes/default/ideation");
    std::fs::create_dir_all(&ideation).unwrap();
    std::fs::write(
        ideation.join("proposal_cards.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"proposal_a","proposal_key":"a","proposal_type":"requirement_candidate","title":"A","summary":"A","traceability":{"target":{"artifact_type":"requirement","artifact_id":"req_anchor"},"source_ids":[],"evidence_references":[],"supporting_claim_ids":[]},"promotion_state":"proposed"}).to_string(),
    )
    .unwrap();
    std::fs::write(
        ideation.join("dispositions.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"disposition_a","proposal_id":"proposal_a","decision":"rejected","rationale":"Reviewed","actor":{"identity_type":"human","id":"reviewer"}}).to_string(),
    )
    .unwrap();
    let requirements = repo.join(".provenance/state/scopes/default/requirements");
    std::fs::create_dir_all(&requirements).unwrap();
    std::fs::write(
        requirements.join("req.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"req_anchor","statement":"Anchor","status":"active"}).to_string(),
    )
    .unwrap();
    let manifest = std::fs::read(repo.join(".provenance/state/manifest.json")).unwrap();
    let agents = std::fs::read(repo.join("AGENTS.md")).unwrap();
    let skill = std::fs::read(repo.join(".agents/skills/provenance-shaping/SKILL.md")).unwrap();

    init(&repo, &["--disposition-actor-id", "maintainer"])
        .failure()
        .stderr(predicates::str::contains("repository allowlist"));

    assert_eq!(
        std::fs::read(repo.join(".provenance/state/manifest.json")).unwrap(),
        manifest
    );
    assert_eq!(std::fs::read(repo.join("AGENTS.md")).unwrap(), agents);
    assert_eq!(
        std::fs::read(repo.join(".agents/skills/provenance-shaping/SKILL.md")).unwrap(),
        skill
    );
    assert!(repo
        .join(".provenance/cache/locks/repository.publication.lock")
        .is_file());
}

#[test]
#[verifies("rule_init_validates_planned_repository", examples)]
fn init_recovers_interrupted_publication_before_classifying_the_repository() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    init(&repo, &["--scope", "default"]).success();
    let transaction = repo.join(".provenance/cache/import-transactions/interrupted-init");
    std::fs::create_dir_all(&transaction).unwrap();
    std::fs::rename(
        repo.join(".provenance/state"),
        transaction.join("backup-state"),
    )
    .unwrap();
    std::fs::write(
        repo.join(".provenance/cache/import-publication.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "transaction_dir": transaction,
            "phase": "backup_created"
        }))
        .unwrap(),
    )
    .unwrap();

    init(&repo, &[]).success();

    assert!(repo.join(".provenance/state/manifest.json").is_file());
    assert!(!repo
        .join(".provenance/cache/import-publication.json")
        .exists());
}
