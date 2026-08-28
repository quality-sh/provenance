//! End-to-end RBAC enforcement through the real `provenance` binary.
//!
//! Seeds a live-proposal scope by writing canonical shards directly, then
//! drives mutating commands against manifests in each regime.

use assert_cmd::prelude::*;
use predicates::str::contains;
use std::process::Command;

const MISSING_CLAIM: &str =
    "rbac: no actor claim supplied for a mutating operation on an rbac-managed repository";

fn provenance() -> Command {
    let mut command = Command::cargo_bin("provenance").unwrap();
    command.env("PROVENANCE_SKIP_ONBOARDING", "1");
    command
}

fn init_repo(directory: &std::path::Path) {
    provenance()
        .args([
            "init",
            "--path",
            directory.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
}

fn manifest_path(directory: &std::path::Path) -> std::path::PathBuf {
    directory.join(".provenance/state/manifest.json")
}

/// Replaces the manifest wholesale; the JSON must be a complete manifest.
fn install_manifest(directory: &std::path::Path, body: &str) {
    std::fs::write(manifest_path(directory), body).unwrap();
}

fn grants(assignments: &str) -> String {
    format!(
        r#"{{
        "schema_version": 1,
        "scopes": [{{"id": "default", "path_prefix": "."}}],
        "disposition_actor_ids": [],
        "rbac": {assignments}
    }}"#
    )
}

fn reviewer_human(capabilities: &str) -> String {
    format!(
        r#"{{"assignments": [{{"actor_id": "reviewer", "identity_type": "human",
            "capabilities": [{capabilities}], "scopes": ["default"]}}]}}"#
    )
}

/// Seeds a scope holding a live (asserted) proposal by writing canonical
/// shards directly, so disposition tests can drive the write path alone.
fn seed_live_proposal(directory: &std::path::Path) {
    let scopes = directory.join(".provenance/state/scopes/default");
    for dir in ["ideation", "requirements", "sources"] {
        std::fs::create_dir_all(scopes.join(dir)).unwrap();
    }
    std::fs::write(
        scopes.join("requirements/req.jsonl"),
        r#"{"schema_version":1,"scope_id":"default","id":"req_overtime","declared_by":null,"declaration_address":null,"retired":false,"statement":"Overtime must be traceable","description":null,"fog":null,"status":"active","domain_id":null,"source_refs":[],"origin_thread":null,"origin_message":null}
"#,
    )
    .unwrap();
    std::fs::write(
        scopes.join("sources/source.jsonl"),
        r#"{"schema_version":1,"scope_id":"default","id":"source_policy","declared_by":null,"declaration_address":null,"retired":false,"name":"Policy","source_type":"policy","url":null,"reference":null,"commit_pin":null,"effective_date":null,"review_date":null,"superseded_by":null,"origin_thread":null,"origin_message":null}
"#,
    )
    .unwrap();
    std::fs::write(
        scopes.join("ideation/contributions.jsonl"),
        r#"{"schema_version":1,"scope_id":"default","id":"contribution_overtime","target":{"artifact_type":"requirement","artifact_id":"req_overtime"},"participant_slot":"reviewer","stance":"support","strongest_finding":"Observed","evidence_references":[{"reference_id":"evidence_overtime","evidence_type":"source","summary":"Pinned"}],"material_claims":[{"claim_id":"claim_overtime","statement":"Observed","evidence_type":"source","evidence_reference_ids":["evidence_overtime"],"confidence":null}],"risks":[],"objections":[],"challenges":[],"suggested_artifact_changes":[],"unsupported_recommendations":[],"uncertainty":{"level":"low","rationale":"Direct"},"open_questions":[]}
"#,
    )
    .unwrap();
    std::fs::write(
        scopes.join("ideation/synthesis_packets.jsonl"),
        r#"{"schema_version":1,"scope_id":"default","id":"synthesis_overtime","target":{"artifact_type":"requirement","artifact_id":"req_overtime"},"summary":"Adjudicated","consensus":[],"contested_claims":[],"minority_objections":[],"evidence_gaps":[],"unsupported_speculation":[],"open_questions":[],"suggested_artifacts":[{"proposal_id":"proposal_overtime","proposal_key":"overtime","proposal_type":"requirement_candidate","summary":"Candidate","origin_participant_slots":["reviewer"]}],"required_human_decisions":[]}
"#,
    )
    .unwrap();
    std::fs::write(
        scopes.join("ideation/proposal_cards.jsonl"),
        r#"{"schema_version":1,"scope_id":"default","id":"proposal_overtime","proposal_key":"overtime","proposal_type":"requirement_candidate","title":"Overtime","summary":"Candidate","confidence":null,"traceability":{"target":{"artifact_type":"requirement","artifact_id":"req_overtime"},"source_ids":[],"evidence_references":[],"supporting_claim_ids":["claim_overtime"]},"promotion_state":"proposed","builds_on":[],"duplicate_of":null,"superseded_by":null}
"#,
    )
    .unwrap();
    std::fs::write(
        scopes.join("ideation/assertions.jsonl"),
        r#"{"schema_version":1,"scope_id":"default","id":"assertion_overtime","proposal_id":"proposal_overtime","synthesis_packet_id":"synthesis_overtime","supporting_claim_ids":["claim_overtime"]}
"#,
    )
    .unwrap();
}

fn create_source_assert(
    directory: &std::path::Path,
    actor: Option<&str>,
) -> assert_cmd::assert::Assert {
    let mut command = provenance();
    command.args([
        "sources",
        "create",
        "--repo",
        directory.to_str().unwrap(),
        "--scope",
        "default",
        "--id",
        "source_added",
        "--name",
        "Added",
    ]);
    if let Some(actor) = actor {
        command.arg("--actor-id").arg(actor);
    }
    command.assert()
}

#[test]
fn a_mutation_without_a_claim_refuses_with_the_missing_claim_golden() {
    let directory = tempfile::tempdir().unwrap();
    init_repo(directory.path());
    install_manifest(directory.path(), &grants(&reviewer_human("\"edit\"")));

    create_source_assert(directory.path(), None)
        .failure()
        .stderr(contains(MISSING_CLAIM));
}

#[test]
fn a_mutation_by_a_wrong_principal_refuses_naming_scope_and_capability() {
    let directory = tempfile::tempdir().unwrap();
    init_repo(directory.path());
    install_manifest(directory.path(), &grants(&reviewer_human("\"edit\"")));

    create_source_assert(directory.path(), Some("intruder"))
        .failure()
        .stderr(contains(
            "rbac: actor intruder does not hold capability edit on scope default",
        ));
}

#[test]
fn a_granted_principal_mutates_through_the_cli() {
    let directory = tempfile::tempdir().unwrap();
    init_repo(directory.path());
    install_manifest(directory.path(), &grants(&reviewer_human("\"edit\"")));

    create_source_assert(directory.path(), Some("reviewer")).success();
}

#[test]
fn repositories_without_the_section_keep_taking_claimless_mutations() {
    let directory = tempfile::tempdir().unwrap();
    init_repo(directory.path());

    create_source_assert(directory.path(), None).success();
}

#[test]
fn cross_scope_writes_refuse_through_the_cli() {
    let directory = tempfile::tempdir().unwrap();
    init_repo(directory.path());
    install_manifest(
        directory.path(),
        &grants(&reviewer_human_scoped("\"edit\"", "\"docs\"")),
    );

    create_source_assert(directory.path(), Some("reviewer"))
        .failure()
        .stderr(contains(
            "rbac: actor reviewer does not hold capability edit on scope default",
        ));
}

fn reviewer_human_scoped(capabilities: &str, scopes: &str) -> String {
    format!(
        r#"{{"assignments": [{{"actor_id": "reviewer", "identity_type": "human",
            "capabilities": [{capabilities}], "scopes": [{scopes}]}}]}}"#
    )
}

const AMBIGUOUS: &str = "ambiguous manifest: disposition_actor_ids and rbac.assignments are both present; move disposition actors into rbac assignments and remove disposition_actor_ids";

const RATIFICATION: &str =
    "rbac: disposition actor reviewer needs an assignment with identity_type human to end a live proposal";

fn dispositions_create(
    directory: &std::path::Path,
    id: &str,
    actor_type: &str,
) -> assert_cmd::assert::Assert {
    provenance()
        .current_dir(directory)
        .args(["--actor-id", "reviewer"])
        .args([
            "dispositions",
            "create",
            "--scope",
            "default",
            "--id",
            id,
            "--proposal-id",
            "proposal_overtime",
            "--decision",
            "accepted",
            "--rationale",
            "Reviewed",
            "--actor-type",
            actor_type,
            "--actor-id",
            "reviewer",
        ])
        .assert()
}

#[test]
fn dispositions_demand_a_human_typed_assignment_on_rbac_repositories() {
    let directory = tempfile::tempdir().unwrap();
    init_repo(directory.path());
    seed_live_proposal(directory.path());
    install_manifest(
        directory.path(),
        &grants(
            r#"{"assignments": [
                {"actor_id": "reviewer", "identity_type": "human", "capabilities": ["edit", "execute"], "scopes": ["default"]},
                {"actor_id": "robot", "identity_type": "agent", "capabilities": ["execute"], "scopes": ["default"]}
            ]}"#,
        ),
    );

    // The recorded actor resolves to an agent-typed assignment: refused.
    provenance()
        .current_dir(directory.path())
        .args(["--actor-id", "reviewer"])
        .args([
            "dispositions",
            "create",
            "--scope",
            "default",
            "--id",
            "disposition_robot",
            "--proposal-id",
            "proposal_overtime",
            "--decision",
            "accepted",
            "--rationale",
            "Reviewed",
            "--actor-type",
            "agent",
            "--actor-id",
            "robot",
        ])
        .assert()
        .failure()
        .stderr(contains(
            "rbac: disposition actor robot needs an assignment with identity_type human to end a live proposal",
        ));

    dispositions_create(directory.path(), "disposition_human", "human").success();
}

#[test]
fn an_assignment_without_identity_type_fails_closed_for_ratification() {
    let directory = tempfile::tempdir().unwrap();
    init_repo(directory.path());
    seed_live_proposal(directory.path());
    install_manifest(
        directory.path(),
        &grants(
            r#"{"assignments": [{"actor_id": "reviewer", "capabilities": ["edit", "execute"], "scopes": ["default"]}]}"#,
        ),
    );

    dispositions_create(directory.path(), "disposition_overtime", "human")
        .failure()
        .stderr(contains(RATIFICATION));
}

#[test]
fn a_legacy_only_repository_keeps_its_exact_allowlist_law() {
    let directory = tempfile::tempdir().unwrap();
    init_repo(directory.path());
    seed_live_proposal(directory.path());
    install_manifest(
        directory.path(),
        r#"{
        "schema_version": 1,
        "scopes": [{"id": "default", "path_prefix": "."}],
        "disposition_actor_ids": []
    }"#,
    );

    provenance()
        .current_dir(directory.path())
        .args([
            "dispositions",
            "create",
            "--scope",
            "default",
            "--id",
            "disposition_overtime",
            "--proposal-id",
            "proposal_overtime",
            "--decision",
            "accepted",
            "--rationale",
            "Reviewed",
            "--actor-type",
            "human",
            "--actor-id",
            "reviewer",
        ])
        .assert()
        .failure()
        .stderr(contains("no disposition actors configured: repository manifest disposition_actor_ids is empty; set it with provenance init --disposition-actor-id <ACTOR_ID>"));
}

#[test]
fn a_manifest_holding_both_regimes_refuses_every_read_and_write() {
    let directory = tempfile::tempdir().unwrap();
    init_repo(directory.path());
    install_manifest(
        directory.path(),
        r#"{
        "schema_version": 1,
        "scopes": [{"id": "default", "path_prefix": "."}],
        "disposition_actor_ids": ["ben"],
        "rbac": {"assignments": []}
    }"#,
    );

    provenance()
        .args(["check", "--repo", directory.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains(AMBIGUOUS));

    provenance()
        .args([
            "sources",
            "create",
            "--repo",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            "source_any",
            "--name",
            "Any",
        ])
        .assert()
        .failure()
        .stderr(contains(AMBIGUOUS));
}

#[test]
fn reinit_of_an_rbac_repository_demands_manifest_write_on_every_scope() {
    let directory = tempfile::tempdir().unwrap();
    init_repo(directory.path());
    install_manifest(
        directory.path(),
        &grants(
            r#"{"assignments": [
                {"actor_id": "operator", "identity_type": "human", "capabilities": ["read", "edit", "execute", "manifest-write"], "scopes": ["default"]},
                {"actor_id": "reader", "identity_type": "human", "capabilities": ["read"], "scopes": ["default"]}
            ]}"#,
        ),
    );

    provenance()
        .args(["init", "--path", directory.path().to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains(MISSING_CLAIM));

    provenance()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--actor-id",
            "reader",
        ])
        .assert()
        .failure()
        .stderr(contains(
            "rbac: actor reader does not hold capability manifest-write on scope default",
        ));

    provenance()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--actor-id",
            "operator",
        ])
        .assert()
        .success();
}

#[test]
fn reinit_with_flags_omitted_preserves_the_rbac_section() {
    let directory = tempfile::tempdir().unwrap();
    init_repo(directory.path());
    let section = grants(&reviewer_human(
        "\"read\",\"edit\",\"execute\",\"manifest-write\"",
    ));
    install_manifest(directory.path(), &section);
    provenance()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--actor-id",
            "reviewer",
        ])
        .assert()
        .success();

    let after = std::fs::read_to_string(manifest_path(directory.path())).unwrap();
    let before: serde_json::Value = serde_json::from_str(&section).unwrap();
    let after: serde_json::Value = serde_json::from_str(&after).unwrap();
    assert_eq!(
        after, before,
        "the rbac section must survive re-init unchanged"
    );
}

#[test]
fn init_disposition_actor_flags_print_the_window_deprecation_warning() {
    let directory = tempfile::tempdir().unwrap();
    provenance()
        .args([
            "init",
            "--path",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--disposition-actor-id",
            "reviewer",
        ])
        .assert()
        .success()
        .stderr(contains(
            "warning: init --disposition-actor-id / --clear-disposition-actors are deprecated",
        ))
        .stderr(contains("rbac.assignments"));

    let fresh = tempfile::tempdir().unwrap();
    provenance()
        .args([
            "init",
            "--path",
            fresh.path().to_str().unwrap(),
            "--scope",
            "default",
        ])
        .assert()
        .success()
        .stderr(predicates::str::is_empty());
}
