use super::*;
use crate::{
    layout::ProvenanceLayout,
    operations::catalog::{
        Operation, PreparedContext, PreparedScope, WriteDiscussionRequest,
        WriteTargetDiscussionRequest,
    },
    state_store::StateStore,
};
use provenance_core::{Manifest, RepoPathPrefix, ScopeId};
use serde_json::json;

fn fixture() -> (tempfile::TempDir, PreparedContext, StateStore, ScopeId) {
    let temp = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root.clone());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let scope = ScopeId::new("default").unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&Manifest::default_with_scope(
            scope.clone(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    let context = PreparedContext::for_scope(PreparedScope {
        root,
        scope: scope.clone(),
        requested_target: "fixture".into(),
    });
    (temp, context, StateStore::new(layout), scope)
}

fn create_request(_request_id: &str) -> CreateRequirementRequest {
    serde_json::from_value(json!({
        "actor": "ben",
        "id": "req_a",
        "statement": "The system stores records.",
        "description": null,
        "status": "discovery",
        "domain_id": null,
        "refines": null,
        "depends_on": [],
        "supersedes": [],
        "spawned_by": null,
        "origin_thread": null,
        "origin_message": null,
        "origin": null
    }))
    .unwrap()
}

fn update_request(store: &StateStore, request_id: &str) -> UpdateRequirementRequest {
    let scope = ScopeId::new("default").unwrap();
    let id = StableId::new("req_a").unwrap();
    update_request_with_etag(
        request_id,
        &store.requirement_edit_state(&scope, &id).unwrap().etag,
        "Saved text.",
    )
}

fn update_request_with_etag(
    _request_id: &str,
    expected_etag: &str,
    description: &str,
) -> UpdateRequirementRequest {
    serde_json::from_value(json!({
        "actor": "ben",
        "expected_etag": expected_etag,
        "declared_by": null,
        "statement": null,
        "description": description,
        "fog": null,
        "status": null,
        "domain_id": null,
        "clear_fields": [],
        "relationships": null,
        "id": "req_a"
    }))
    .unwrap()
}

#[tokio::test]
async fn create_response_failure_refuses_before_publication() {
    let (_temp, context, store, scope) = fixture();
    crate::test_probes::arm("requirement_resource_snapshot", || {
        anyhow::bail!("injected response construction failure")
    });

    let result = CreateRequirementResource::run(context, create_request("create_a")).await;
    crate::test_probes::disarm("requirement_resource_snapshot");

    assert!(result.is_err());
    assert_eq!(
        store.list_requirements(&scope).unwrap(),
        [] as [provenance_core::Requirement; 0]
    );
    assert_eq!(
        store.review_entries(&scope).unwrap(),
        [] as [provenance_core::review::ReviewEntry; 0]
    );

    let committed = CreateRequirementResource::run(
        PreparedContext::for_scope(PreparedScope {
            root: store.layout.root().to_owned(),
            scope: scope.clone(),
            requested_target: "fixture".into(),
        }),
        create_request("create_a"),
    )
    .await
    .unwrap();
    assert_eq!(committed.record.id.as_str(), "req_a");
    assert_eq!(
        committed.decision.pending.as_ref().unwrap().revision,
        committed.edit.revision.clone().unwrap()
    );
    assert_eq!(store.review_entries(&scope).unwrap().len(), 1);
}

#[tokio::test]
async fn content_update_replaces_or_opens_the_current_submission() {
    let (_temp, context, store, scope) = fixture();
    let created = CreateRequirementResource::run(context.clone(), create_request("create_a"))
        .await
        .unwrap();
    let first = created.decision.pending.unwrap();

    let updated =
        UpdateRequirementResource::run(context.clone(), update_request(&store, "update_a"))
            .await
            .unwrap();
    let second = updated.decision.pending.unwrap();
    assert_ne!(second.proposal_id, first.proposal_id);
    assert_ne!(second.revision, first.revision);
    assert_eq!(second.revision, updated.edit.revision.unwrap());

    store
        .withdraw_record_review(
            serde_json::from_value(json!({
                "scope_id":"default", "actor":"ben",
                "proposal_id":second.proposal_id
            }))
            .unwrap(),
        )
        .unwrap();
    let etag = store
        .requirement_edit_state(&scope, &StableId::new("req_a").unwrap())
        .unwrap()
        .etag;
    let third = UpdateRequirementResource::run(
        context,
        update_request_with_etag("update_b", &etag, "New text."),
    )
    .await
    .unwrap();
    assert_ne!(
        third.decision.pending.unwrap().proposal_id,
        second.proposal_id
    );
    assert_eq!(
        store
            .requirement_decision_state(&scope, &StableId::new("req_a").unwrap())
            .unwrap()
            .decisions
            .len(),
        0
    );
}

#[tokio::test]
async fn lifecycle_update_keeps_the_current_submission() {
    let (_temp, context, store, _scope) = fixture();
    let created = CreateRequirementResource::run(context.clone(), create_request("create_a"))
        .await
        .unwrap();
    let pending = created.decision.pending.unwrap();
    let request: UpdateRequirementRequest = serde_json::from_value(json!({
        "actor":"ben", "expected_etag":created.edit.etag,
        "declared_by":null, "statement":null, "description":null, "fog":null,
        "status":"active", "domain_id":null, "clear_fields":[],
        "relationships":null, "id":"req_a"
    }))
    .unwrap();

    let updated = UpdateRequirementResource::run(context, request)
        .await
        .unwrap();

    assert_eq!(updated.decision.pending.unwrap(), pending);
    assert_eq!(updated.edit.revision.unwrap(), pending.revision);
    assert_eq!(
        store
            .list_proposal_definitions(&ScopeId::new("default").unwrap())
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn revision_keeps_the_prior_submission_and_feedback_readable() {
    let (_temp, context, store, scope) = fixture();
    let created = CreateRequirementResource::run(context.clone(), create_request("create_a"))
        .await
        .unwrap();
    let proposal = created.decision.pending.unwrap().proposal_id;
    let mut manifest = store.manifest().unwrap();
    manifest.disposition_actor_ids.push("reviewer".to_owned());
    std::fs::write(
        store.layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    store
        .decide_record_review(
            serde_json::from_value(json!({
                "scope_id":"default",
                "actor":{"identity_type":"human","id":"reviewer"},
                "proposal_id":proposal,
                "decision":"rejected",
                "rationale":"The statement needs more detail.",
                "canonical_artifact":null,
                "feedback":{"role":"user","body":"Add the missing condition."},
                "declared_by":null
            }))
            .unwrap(),
        )
        .unwrap();

    let revised = UpdateRequirementResource::run(context, update_request(&store, "update_a"))
        .await
        .unwrap();

    assert_ne!(
        revised.decision.pending.as_ref().unwrap().proposal_id,
        proposal
    );
    assert_eq!(revised.decision.decisions.len(), 1);
    assert!(revised.decision.decisions[0].feedback_message_id.is_some());
    assert!(store
        .list_proposal_definitions(&scope)
        .unwrap()
        .iter()
        .any(|record| record.id == proposal));
}

#[tokio::test]
async fn update_response_failure_refuses_before_publication() {
    let (_temp, context, store, scope) = fixture();
    store
        .create_requirement(
            serde_json::from_value(json!({
                "scope_id": "default",
                "id": "req_a",
                "statement": "The system stores records.",
                "status": "discovery",
                "depends_on": [],
                "supersedes": []
            }))
            .unwrap(),
        )
        .unwrap();
    let request = update_request(&store, "update_a");
    let receipts_before = store.review_entries(&scope).unwrap();
    crate::test_probes::arm("requirement_resource_snapshot", || {
        anyhow::bail!("injected response construction failure")
    });

    let result = UpdateRequirementResource::run(context, request).await;
    crate::test_probes::disarm("requirement_resource_snapshot");

    assert!(result.is_err());
    let record = store.list_requirements(&scope).unwrap().remove(0);
    assert_eq!(record.description, None);
    assert_eq!(store.review_entries(&scope).unwrap(), receipts_before);

    let committed = UpdateRequirementResource::run(
        PreparedContext::for_scope(PreparedScope {
            root: store.layout.root().to_owned(),
            scope: scope.clone(),
            requested_target: "fixture".into(),
        }),
        update_request(&store, "update_a"),
    )
    .await
    .unwrap();
    assert_eq!(committed.record.description.as_deref(), Some("Saved text."));
    assert_eq!(
        store.review_entries(&scope).unwrap().len(),
        receipts_before.len() + 1
    );
}

#[tokio::test]
async fn a_repeated_update_with_an_old_etag_returns_a_typed_conflict() {
    let (_temp, context, store, scope) = fixture();
    CreateRequirementResource::run(context.clone(), create_request("create_a"))
        .await
        .unwrap();
    let id = StableId::new("req_a").unwrap();
    let first_etag = store.requirement_edit_state(&scope, &id).unwrap().etag;
    UpdateRequirementResource::run(
        context.clone(),
        update_request_with_etag("update_a", &first_etag, "First text."),
    )
    .await
    .unwrap();
    let second_etag = store.requirement_edit_state(&scope, &id).unwrap().etag;
    UpdateRequirementResource::run(
        context.clone(),
        update_request_with_etag("update_b", &second_etag, "Second text."),
    )
    .await
    .unwrap();

    let Err(error) = UpdateRequirementResource::run(
        context,
        update_request_with_etag("update_a", &first_etag, "First text."),
    )
    .await
    else {
        panic!("the stale update succeeded");
    };

    assert!(matches!(
        error.safe(),
        crate::write_error::WriteFailure::RequirementEditConflict { .. }
    ));
    assert_eq!(store.review_entries(&scope).unwrap().len(), 3);
}

#[test]
fn concurrent_resource_writes_with_one_etag_commit_once() {
    let (_temp, _context, store, scope) = fixture();
    store
        .create_review_requirement(
            serde_json::from_value(json!({
                "request_id": "create_a",
                "actor": "ben",
                "origin": null,
                "create": {
                    "scope_id": "default",
                    "id": "req_a",
                    "statement": "The system stores records.",
                    "status": "discovery",
                    "depends_on": [],
                    "supersedes": []
                }
            }))
            .unwrap(),
        )
        .unwrap();
    let id = StableId::new("req_a").unwrap();
    let etag = store.requirement_edit_state(&scope, &id).unwrap().etag;
    let input = |request: &str, description: &str| {
        serde_json::from_value(json!({
            "request_id": request,
            "actor": "ben",
            "expected_etag": etag,
            "update": {
                "scope_id": "default",
                "id": "req_a",
                "description": description
            },
            "relationships": null
        }))
        .unwrap()
    };
    let first = input("update_a", "First text.");
    let second = input("update_b", "Second text.");

    std::thread::scope(|threads| {
        let first = threads.spawn(|| store.save_requirement_resource(first));
        let second = threads.spawn(|| store.save_requirement_resource(second));
        let results = [first.join().unwrap(), second.join().unwrap()];
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert!(results
            .iter()
            .find_map(|result| result.as_ref().err())
            .unwrap()
            .to_string()
            .contains("etag"));
    });
    assert_eq!(store.review_entries(&scope).unwrap().len(), 2);
}

#[test]
#[provenance_macros::verifies("rule_review_request_identity_server_created", examples)]
fn review_action_requests_exclude_server_created_identities() {
    let submit = serde_json::from_value::<review::SubmitRecordReview>(json!({
        "scope_id":"default", "actor":"agent", "record_kind":"requirement", "record_id":"req_a",
        "title":"Title", "summary":"Summary", "source_ids":[],
        "evidence_references":[], "builds_on":[],
        "expected_revision":null, "revises":null
    }));
    assert!(submit.is_ok());

    let decide = serde_json::from_value::<DecideRecordReviewRequest>(json!({
        "scope_id":"default", "record_kind":"requirement", "record_id":"req_a", "actor":{
            "identity_type":"human", "id":"reviewer"
        }, "proposal_id":"prop-1", "decision":"accepted", "rationale":null,
        "canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_a"},
        "feedback":null, "declared_by":null
    }));
    assert!(decide.is_ok());

    let withdraw = serde_json::from_value::<WithdrawRecordReviewRequest>(json!({
        "scope_id":"default", "record_kind":"requirement", "record_id":"req_a", "actor":"agent",
        "proposal_id":"prop-1", "declared_by":null, "reason":null
    }));
    assert!(withdraw.is_ok());

    for (field, supplied) in [
        ("request_id", json!("client-request")),
        ("proposal_id", json!("client-proposal")),
        ("proposal_key", json!("client-key")),
    ] {
        let mut value = json!({
            "scope_id":"default", "actor":"agent", "record_kind":"requirement", "record_id":"req_a",
            "title":"Title", "summary":"Summary", "source_ids":[],
            "evidence_references":[], "builds_on":[], "expected_revision":null, "revises":null
        });
        value[field] = supplied;
        assert!(serde_json::from_value::<review::SubmitRecordReview>(value).is_err());
    }
    assert!(
        serde_json::from_value::<DecideRecordReviewRequest>(json!({
            "scope_id":"default", "record_kind":"requirement", "record_id":"req_a", "request_id":"client-request",
            "actor":{"identity_type":"human", "id":"reviewer"}, "proposal_id":"prop-1",
            "disposition_id":"client-disposition", "decision":"accepted", "rationale":null,
            "canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_a"},
            "feedback":null, "declared_by":null
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<WithdrawRecordReviewRequest>(json!({
            "scope_id":"default", "record_kind":"requirement", "record_id":"req_a",
            "request_id":"client-request", "actor":"agent", "proposal_id":"prop-1",
            "declared_by":null, "reason":null
        }))
        .is_err()
    );
}

#[test]
#[provenance_macros::verifies("rule_review_request_identity_server_created", examples)]
fn remaining_review_write_requests_exclude_client_request_identities() {
    let create = json!({
        "actor":"agent", "id":"req_a", "statement":"One statement.",
        "description":null, "status":"discovery", "domain_id":null,
        "refines":null, "depends_on":[], "supersedes":[], "spawned_by":null,
        "origin_thread":null, "origin_message":null, "origin":null
    });
    assert!(serde_json::from_value::<CreateRequirementRequest>(create.clone()).is_ok());

    let update = json!({
        "actor":"agent", "expected_etag":"etag", "declared_by":null,
        "statement":null, "description":"New text.", "fog":null, "status":null,
        "domain_id":null, "clear_fields":[], "relationships":null, "id":"req_a"
    });
    assert!(serde_json::from_value::<UpdateRequirementRequest>(update.clone()).is_ok());

    let discussion = json!({
        "scope_id":"default", "parent":{
            "node_type":"requirement", "node_id":"req_a"
        }, "actor":"agent", "declared_by":null,
        "action":{"kind":"start", "role":"user", "body":"Concern."}
    });
    assert!(serde_json::from_value::<WriteDiscussionRequest>(discussion.clone()).is_ok());

    let reply = json!({
        "scope_id":"default", "actor":"agent", "declared_by":null,
        "allowed_parent_kinds":["requirement"], "discussion_id":"discussion_a",
        "expected_version":1, "role":"user", "body":"Reply."
    });
    assert!(serde_json::from_value::<WriteTargetDiscussionRequest>(reply.clone()).is_ok());

    let mut create_with_identity = create;
    create_with_identity["request_id"] = json!("client-request");
    assert!(serde_json::from_value::<CreateRequirementRequest>(create_with_identity).is_err());

    let mut update_with_identity = update;
    update_with_identity["request_id"] = json!("client-request");
    assert!(serde_json::from_value::<UpdateRequirementRequest>(update_with_identity).is_err());

    let mut discussion_with_identity = discussion;
    discussion_with_identity["request_id"] = json!("client-request");
    assert!(serde_json::from_value::<WriteDiscussionRequest>(discussion_with_identity).is_err());

    let mut reply_with_identity = reply;
    reply_with_identity["request_id"] = json!("client-request");
    assert!(serde_json::from_value::<WriteTargetDiscussionRequest>(reply_with_identity).is_err());
}
