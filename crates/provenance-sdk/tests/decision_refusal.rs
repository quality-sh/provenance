//! A review decision through the SDK operations by an actor that the manifest
//! field `disposition_actor_ids` does not contain gets the `access_denied`
//! refusal that names the field and the init command.

use provenance_core::protocol::failure::OperationError;
use provenance_core::{Manifest, RepoPathPrefix, ScopeId};
use provenance_macros::verifies;
use provenance_sdk::operations::catalog::{
    invoke_typed, CreateRequirementResource, DecideRecordReview, PreparedContext, PreparedScope,
};
use provenance_sdk::Repository;
use provenance_store::layout::ProvenanceLayout;
use serde_json::{json, Value};

const NEXT_STEP: &str = "To change this field, use the command provenance init \
    --disposition-actor-id <DISPOSITION_ACTOR_ID>. Give --disposition-actor-id one time for each \
    actor that can make a review decision.";

/// A repository whose manifest field holds `actor_ids`.
fn repository(actor_ids: &[&str]) -> (tempfile::TempDir, Repository) {
    let dir = tempfile::tempdir().unwrap();
    let root =
        camino::Utf8PathBuf::from_path_buf(dunce::canonicalize(dir.path()).unwrap()).unwrap();
    let layout = ProvenanceLayout::new(root.clone());
    std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
    let mut manifest =
        Manifest::default_with_scope(ScopeId::new("default").unwrap(), RepoPathPrefix::new("."));
    manifest.disposition_actor_ids = actor_ids.iter().map(|id| (*id).to_owned()).collect();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_string(&manifest).unwrap(),
    )
    .unwrap();
    let repository = Repository::open(&root, "default").unwrap();
    (dir, repository)
}

fn context(repository: &Repository) -> PreparedContext {
    PreparedContext::for_scope(PreparedScope {
        repository: repository.clone(),
        requested_target: "selected".into(),
    })
}

/// The answer to a decision by `stranger` on a new pending submission.
async fn refused_decision(actor_ids: &[&str]) -> Value {
    let (_dir, repository) = repository(actor_ids);
    let created = invoke_typed::<CreateRequirementResource>(
        context(&repository),
        serde_json::from_value(json!({
            "actor": "author", "id": "req_review", "statement": "The system stores records.",
            "description": null, "status": "discovery", "depends_on": [], "supersedes": [],
            "spawned_by": null, "origin_thread": null, "origin_message": null, "origin": null
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    let proposal = created.decision.pending.unwrap().proposal_id;
    let refused = invoke_typed::<DecideRecordReview>(
        context(&repository),
        serde_json::from_value(json!({
            "scope_id": "default", "record_kind": "requirement", "record_id": "req_review",
            "actor": {"identity_type": "human", "id": "stranger"}, "proposal_id": proposal,
            "decision": "rejected", "rationale": "The statement needs a scope.",
            "canonical_artifact": null, "feedback": null, "declared_by": null
        }))
        .unwrap(),
    )
    .await
    .unwrap_err();
    let OperationError::Handler(error) = refused else {
        panic!("{refused:?}");
    };
    serde_json::to_value(error.safe()).unwrap()
}

#[tokio::test]
#[verifies("rule_decision_refusal_names_reviewer_field", examples)]
#[verifies("rule_decision_refusal_hides_reviewer_ids", examples)]
async fn an_sdk_decision_refusal_names_the_field_when_the_field_holds_other_actors() {
    let message = format!(
        "The manifest field disposition_actor_ids does not contain the actor of a review \
         decision. {NEXT_STEP}"
    );
    assert_eq!(
        refused_decision(&["lead_reviewer"]).await,
        json!({"kind": "access_denied", "message": message})
    );
}

#[tokio::test]
#[verifies("rule_decision_refusal_names_reviewer_field", examples)]
#[verifies("rule_decision_refusal_hides_reviewer_ids", examples)]
async fn an_sdk_decision_refusal_names_the_field_when_the_field_is_empty() {
    let message = format!(
        "The manifest field disposition_actor_ids is empty, thus no actor can make a review \
         decision. {NEXT_STEP}"
    );
    assert_eq!(
        refused_decision(&[]).await,
        json!({"kind": "access_denied", "message": message})
    );
}
