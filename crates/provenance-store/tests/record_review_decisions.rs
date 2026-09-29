use provenance_core::{
    review::CycleFact, CanonicalArtifactType, DispositionActor, DispositionDecision, IdentityType,
    NodeType, ScopeId, StableId,
};
use provenance_store::{
    layout::ProvenanceLayout,
    review::{DecideRecordReview, ReviewFeedback, SubmitRecordReview, WithdrawRecordReview},
    state_store::{CreateDomainInput, CreateSourceInput, StateStore, UpdateSourceInput},
    write_error::{WriteError, WriteFailure},
};
use serde_json::json;

fn fixture() -> (tempfile::TempDir, StateStore, ScopeId) {
    let temp = tempfile::tempdir().unwrap();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}],"disposition_actor_ids":["reviewer"]}"#,
    )
    .unwrap();
    let scope = ScopeId::new("default").unwrap();
    (temp, StateStore::new(layout), scope)
}

fn id(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn reviewer() -> DispositionActor {
    DispositionActor {
        identity_type: IdentityType::Human,
        id: "reviewer".into(),
        name: None,
    }
}

fn create_source(store: &StateStore) -> StableId {
    let source = id("source_a");
    store
        .create_source(
            serde_json::from_value::<CreateSourceInput>(json!({
                "scope_id":"default", "id":source, "name":"Policy A",
                "source_type":"policy", "supersedes":[]
            }))
            .unwrap(),
        )
        .unwrap();
    source
}

#[test]
fn source_cycle_binds_each_decision_to_the_exact_revision() {
    let (_temp, store, scope) = fixture();
    let source = create_source(&store);
    let created = store
        .record_decision_state(&scope, NodeType::Source, &source)
        .unwrap();
    let first = created.pending.unwrap();

    store
        .withdraw_record_review(WithdrawRecordReview {
            scope_id: scope.clone(),
            actor: "author".into(),
            proposal_id: first.proposal_id.clone(),
            declared_by: None,
            reason: Some("The author will resubmit it.".into()),
        })
        .unwrap();
    let submitted = store
        .submit_record_review(SubmitRecordReview {
            scope_id: scope.clone(),
            actor: "author".into(),
            record_kind: NodeType::Source,
            record_id: source.clone(),
            declared_by: None,
            title: "Review Policy A".into(),
            summary: "Review the source definition.".into(),
            confidence: None,
            source_ids: Vec::new(),
            evidence_references: Vec::new(),
            builds_on: Vec::new(),
            expected_revision: created.current_revision.clone(),
            revises: None,
        })
        .unwrap();
    store
        .decide_record_review(DecideRecordReview {
            scope_id: scope.clone(),
            actor: reviewer(),
            proposal_id: submitted.proposal_id,
            decision: DispositionDecision::Rejected,
            rationale: Some("The source name is not precise.".into()),
            canonical_artifact: None,
            feedback: Some(ReviewFeedback {
                role: provenance_core::MessageRole::User,
                body: "Use the policy's full name.".into(),
            }),
            declared_by: None,
        })
        .unwrap();

    store
        .update_source(
            serde_json::from_value::<UpdateSourceInput>(json!({
                "scope_id":"default", "id":"source_a", "name":"Policy Alpha"
            }))
            .unwrap(),
        )
        .unwrap();
    let revised = store
        .record_decision_state(&scope, NodeType::Source, &source)
        .unwrap();
    let second = revised.pending.clone().unwrap();
    assert_ne!(second.revision, first.revision);

    let before_approval = store.list_sources(&scope).unwrap().remove(0);
    store
        .decide_record_review(DecideRecordReview {
            scope_id: scope.clone(),
            actor: reviewer(),
            proposal_id: second.proposal_id.clone(),
            decision: DispositionDecision::Accepted,
            rationale: None,
            canonical_artifact: Some(provenance_core::CanonicalArtifact {
                artifact_type: CanonicalArtifactType::Source,
                artifact_id: source.clone(),
            }),
            feedback: None,
            declared_by: None,
        })
        .unwrap();
    assert_eq!(store.list_sources(&scope).unwrap()[0], before_approval);

    let decided = store
        .record_decision_state(&scope, NodeType::Source, &source)
        .unwrap();
    assert_eq!(decided.decisions.len(), 2);
    assert_eq!(decided.decisions[0].revision, created.current_revision);
    assert_eq!(decided.decisions[1].revision, Some(second.revision));
    assert_eq!(
        decided.current_acceptance.unwrap().disposition.proposal_id,
        second.proposal_id
    );
    assert_eq!(decided.withdrawn, [first.proposal_id]);
    assert!(store
        .cycle_entries(&scope)
        .unwrap()
        .iter()
        .any(|entry| entry.record_kind == NodeType::Source && entry.fact == CycleFact::Withdrawn));
}

#[test]
fn domain_decision_keeps_rationale_and_refuses_feedback() {
    let (_temp, store, scope) = fixture();
    let domain = id("domain_a");
    store
        .create_domain(CreateDomainInput {
            scope_id: scope.clone(),
            id: domain.clone(),
            name: "Policy".into(),
            description: None,
            color: None,
        })
        .unwrap();
    let proposal = store
        .record_decision_state(&scope, NodeType::Domain, &domain)
        .unwrap()
        .pending
        .unwrap()
        .proposal_id;

    let error = store
        .decide_record_review(DecideRecordReview {
            scope_id: scope.clone(),
            actor: reviewer(),
            proposal_id: proposal.clone(),
            decision: DispositionDecision::Rejected,
            rationale: Some("The classification is too broad.".into()),
            canonical_artifact: None,
            feedback: Some(ReviewFeedback {
                role: provenance_core::MessageRole::User,
                body: "Narrow the classification.".into(),
            }),
            declared_by: None,
        })
        .unwrap_err();

    assert!(matches!(
        WriteError(error).safe(),
        WriteFailure::UnsupportedReviewFeedback {
            record_kind: NodeType::Domain
        }
    ));

    let decided = store
        .decide_record_review(DecideRecordReview {
            scope_id: scope.clone(),
            actor: reviewer(),
            proposal_id: proposal,
            decision: DispositionDecision::Rejected,
            rationale: Some("The classification is too broad.".into()),
            canonical_artifact: None,
            feedback: None,
            declared_by: None,
        })
        .unwrap();
    let disposition = store
        .list_dispositions(&scope)
        .unwrap()
        .into_iter()
        .find(|disposition| Some(&disposition.id) == decided.disposition_id.as_ref())
        .unwrap();
    assert_eq!(disposition.rationale, "The classification is too broad.");
}
