//! The primitive backstop: on an rbac-managed repository, every mutation
//! through the store primitives refuses a missing or unauthorized claim;
//! repositories without the section behave exactly as before.

use super::initialized_store;
use provenance_core::{
    Capability, Manifest, RbacClaim, RepoPathPrefix, ScopeId, SourceType, StableId,
    MISSING_CLAIM_REFUSAL,
};
use serde::{Deserialize, Serialize};

use crate::state_store::{CreateSourceInput, MutationAuth, StateStore};

fn grant(sections: &str) -> String {
    format!(
        r#"{{
        "schema_version": 1,
        "scopes": [{{"id": "default", "path_prefix": "."}}],
        "disposition_actor_ids": [],
        "rbac": {sections}
    }}"#
    )
}

fn reviewer_grants(capabilities: &str, scopes: &str) -> String {
    format!(
        r#"{{"assignments": [{{
            "actor_id": "reviewer",
            "identity_type": "human",
            "capabilities": [{capabilities}],
            "scopes": [{scopes}]
        }}]}}"#
    )
}

fn install_manifest(store: &StateStore, body: String) {
    std::fs::write(store.layout.manifest_path(), body).unwrap();
}

#[allow(clippy::unnecessary_wraps)] // models the CLI's optional claim
fn claim(actor: &str) -> Option<RbacClaim> {
    Some(RbacClaim::new(actor).expect("test actor"))
}

fn source_input(scope: &ScopeId, id: &str) -> CreateSourceInput {
    CreateSourceInput {
        scope_id: scope.clone(),
        id: StableId::new(id).unwrap(),
        name: "Policy".into(),
        source_type: SourceType::Policy,
        url: None,
        reference: None,
        commit_pin: None,
        effective_date: None,
        review_date: None,
        superseded_by: None,
        origin_thread: None,
        origin_message: None,
    }
}

#[test]
fn a_mutation_without_a_claim_refuses_on_an_rbac_repository() {
    let (_dir, store, scope) = initialized_store();
    install_manifest(&store, grant(&reviewer_grants("\"edit\"", "\"default\"")));

    let error = store
        .create_source(None, source_input(&scope, "source_policy"))
        .unwrap_err();
    assert_eq!(error.to_string(), MISSING_CLAIM_REFUSAL);
}

#[test]
fn a_mutation_by_a_wrong_principal_refuses_naming_scope_and_capability() {
    let (_dir, store, scope) = initialized_store();
    install_manifest(&store, grant(&reviewer_grants("\"edit\"", "\"default\"")));

    let error = store
        .create_source(
            claim("intruder").as_ref(),
            source_input(&scope, "source_policy"),
        )
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "rbac: actor intruder does not hold capability edit on scope default"
    );
}

#[test]
fn a_granted_principal_mutates_the_granted_scope() {
    let (_dir, store, scope) = initialized_store();
    install_manifest(&store, grant(&reviewer_grants("\"edit\"", "\"default\"")));

    store
        .create_source(
            claim("reviewer").as_ref(),
            source_input(&scope, "source_policy"),
        )
        .unwrap();
}

#[test]
fn cross_scope_writes_refuse() {
    let (_dir, store, scope) = initialized_store();
    install_manifest(&store, grant(&reviewer_grants("\"edit\"", "\"docs\"")));

    let error = store
        .create_source(
            claim("reviewer").as_ref(),
            source_input(&scope, "source_policy"),
        )
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "rbac: actor reviewer does not hold capability edit on scope default"
    );
}

#[test]
fn a_capability_narrower_than_the_write_refuses() {
    let (_dir, store, scope) = initialized_store();
    install_manifest(&store, grant(&reviewer_grants("\"read\"", "\"default\"")));

    let error = store
        .create_source(
            claim("reviewer").as_ref(),
            source_input(&scope, "source_policy"),
        )
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "rbac: actor reviewer does not hold capability edit on scope default"
    );
    assert_eq!(Capability::Edit.as_str(), "edit");
}

#[test]
fn repositories_without_the_section_keep_taking_claimless_mutations() {
    let (_dir, store, scope) = initialized_store();

    store
        .create_source(None, source_input(&scope, "source_policy"))
        .unwrap();
}

#[test]
fn legacy_only_repositories_keep_taking_claimless_mutations() {
    let (_dir, store, scope) = initialized_store();
    install_manifest(
        &store,
        r#"{
        "schema_version": 1,
        "scopes": [{"id": "default", "path_prefix": "."}],
        "disposition_actor_ids": ["ben"]
    }"#
        .to_string(),
    );

    store
        .create_source(None, source_input(&scope, "source_policy"))
        .unwrap();
}

#[test]
fn a_manifest_with_the_section_and_no_assignments_denies_every_mutation() {
    let (_dir, store, scope) = initialized_store();
    install_manifest(&store, grant(r#"{"assignments": []}"#));

    let error = store
        .create_source(
            claim("reviewer").as_ref(),
            source_input(&scope, "source_policy"),
        )
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "rbac: actor reviewer does not hold capability edit on scope default"
    );
}

#[test]
fn default_deny_at_the_primitive_covers_unregistered_writers() {
    // A synthetic writer standing in for a verb outside the census: it walks
    // the same primitive, so the same backstop refuses it, whatever it is.
    #[derive(Deserialize, Serialize)]
    struct GhostRecord {
        id: String,
    }

    let (_dir, store, _scope) = initialized_store();
    install_manifest(&store, grant(&reviewer_grants("\"edit\"", "\"default\"")));
    let path = store
        .layout
        .state_dir()
        .join("scopes/default/ghosts/ghost-00.jsonl");

    let outcome = store.mutate_jsonl_records::<GhostRecord, ()>(
        &path,
        MutationAuth::new(None, Capability::Edit, &ScopeId::new("default").unwrap()),
        |_| Ok(()),
    );
    assert_eq!(
        outcome.unwrap_err().to_string(),
        MISSING_CLAIM_REFUSAL,
        "an unregistered writer still cannot skip the backstop"
    );

    let outcome = store.mutate_jsonl_records::<GhostRecord, ()>(
        &path,
        MutationAuth::new(
            claim("intruder").as_ref(),
            Capability::Edit,
            &ScopeId::new("default").unwrap(),
        ),
        |_| Ok(()),
    );
    assert_eq!(
        outcome.unwrap_err().to_string(),
        "rbac: actor intruder does not hold capability edit on scope default"
    );

    store
        .mutate_jsonl_records::<GhostRecord, ()>(
            &path,
            MutationAuth::new(
                claim("reviewer").as_ref(),
                Capability::Edit,
                &ScopeId::new("default").unwrap(),
            ),
            |_| Ok(()),
        )
        .unwrap();
}

#[test]
fn reinitializing_a_manifest_layout_keeps_paths_resolvable() {
    // The backstop reads the manifest through the same layout every writer
    // uses; the lock path must keep resolving across writes.
    let (_dir, store, scope) = initialized_store();
    install_manifest(&store, grant(&reviewer_grants("\"edit\"", "\"default\"")));
    store
        .create_source(claim("reviewer").as_ref(), source_input(&scope, "source_a"))
        .unwrap();
    store
        .create_source(claim("reviewer").as_ref(), source_input(&scope, "source_b"))
        .unwrap();
    let manifest: Manifest =
        serde_json::from_str(&std::fs::read_to_string(store.layout.manifest_path()).unwrap())
            .unwrap();
    assert_eq!(manifest.scopes[0].path_prefix, RepoPathPrefix::new("."));
}
