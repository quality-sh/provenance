//! The mutation goldens through the CLI: missing claim, wrong principal,
//! cross-scope, deletion, ratification identity, and the legacy window.

use crate::support::{
    create_source_assert, dispositions_create, grants, init_repo, install_manifest, provenance,
    reviewer_human, reviewer_human_scoped, seed_live_proposal, AMBIGUOUS, MISSING_CLAIM,
    RATIFICATION,
};
use assert_cmd::prelude::*;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

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

/// The endpoints the deletion fixture needs, created by a granted actor.
fn seed_edge_endpoints(directory: &std::path::Path) {
    let repo = directory.to_str().unwrap();
    provenance()
        .args(["--actor-id", "reviewer"])
        .args([
            "sources",
            "create",
            "--repo",
            repo,
            "--scope",
            "default",
            "--id",
            "source_policy",
            "--name",
            "Policy",
        ])
        .assert()
        .success();
    provenance()
        .args(["--actor-id", "reviewer"])
        .args([
            "requirements",
            "create",
            "--repo",
            repo,
            "--scope",
            "default",
            "--id",
            "req_policy",
            "--statement",
            "Policy must be traceable",
        ])
        .assert()
        .success();
}

/// Census row 5: edge deletion is an `edit` mutation like any other, so it
/// refuses without a grant and refuses cross-scope; the granted principal
/// deletes.
#[test]
fn edge_deletion_authorizes_like_any_other_edit() {
    let directory = tempfile::tempdir().unwrap();
    init_repo(directory.path());
    install_manifest(
        directory.path(),
        &grants(&reviewer_human_scoped("\"edit\"", "\"default\"")),
    );

    seed_edge_endpoints(directory.path());
    provenance()
        .args(["--actor-id", "reviewer"])
        .args([
            "edges",
            "create",
            "--repo",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--type",
            "references",
            "--from-type",
            "source",
            "--from-id",
            "source_policy",
            "--to-type",
            "requirement",
            "--to-id",
            "req_policy",
        ])
        .assert()
        .success();

    let delete = |actor: Option<&str>| {
        let mut command = provenance();
        if let Some(actor) = actor {
            command.args(["--actor-id", actor]);
        }
        command
            .args([
                "edges",
                "delete",
                "--repo",
                directory.path().to_str().unwrap(),
                "--scope",
                "default",
                "--id",
                "references_source_source_policy_to_requirement_req_policy",
            ])
            .assert()
    };

    // A claimless deletion refuses and the edge survives.
    delete(None).failure().stderr(contains(MISSING_CLAIM));
    // A principal granted only another scope refuses.
    install_manifest(
        directory.path(),
        &grants(&reviewer_human_scoped("\"edit\"", "\"docs\"")),
    );
    delete(Some("reviewer")).failure().stderr(contains(
        "rbac: actor reviewer does not hold capability edit on scope default",
    ));

    // The granted principal deletes, and the edge is gone.
    install_manifest(
        directory.path(),
        &grants(&reviewer_human_scoped("\"edit\"", "\"default\"")),
    );
    delete(Some("reviewer")).success();
    provenance()
        .args([
            "edges",
            "list",
            "--repo",
            directory.path().to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(contains("references_source_source_policy_to_requirement_req_policy").not());
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
