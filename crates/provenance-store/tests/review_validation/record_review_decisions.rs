use provenance_core::{
    CanonicalArtifactType, DispositionActor, DispositionDecision, IdentityType, NodeType, ScopeId,
    StableId,
};
use provenance_store::{
    layout::ProvenanceLayout,
    review::{DecideRecordReview, ReviewFeedback, SubmitRecordReview},
    state_store::{
        CreateBoundaryInput, CreateDomainInput, CreateQuestionInput, CreateRequirementInput,
        CreateResolutionInput, CreateRuleInput, CreateSourceInput, CreateTopicInput,
        EditQuestionInput, StateStore, UpdateBoundaryInput, UpdateDomainInput,
        UpdateResolutionInput, UpdateRuleInput, UpdateSourceInput, UpdateTopicInput,
    },
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

fn input<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).unwrap()
}

fn create_prerequisites(store: &StateStore, kind: NodeType) {
    if kind != NodeType::Source && kind != NodeType::Domain {
        store
            .create_requirement(input::<CreateRequirementInput>(json!({
                "scope_id":"default", "id":"requirement_a",
                "statement":"The system stores the record.", "status":"active",
                "depends_on":[], "supersedes":[]
            })))
            .unwrap();
    }
    if kind == NodeType::Question {
        store
            .create_topic(input::<CreateTopicInput>(json!({
                "scope_id":"default", "id":"topic_parent",
                "requirement_id":"requirement_a", "title":"Parent topic",
                "status":"open", "links":[]
            })))
            .unwrap();
    }
}

fn create_record(store: &StateStore, kind: NodeType) -> StableId {
    create_prerequisites(store, kind);
    let record_id = id(&format!("{}_a", kind.as_str()));
    match kind {
        NodeType::Source => {
            store
                .create_source(input::<CreateSourceInput>(json!({
                    "scope_id":"default", "id":record_id, "name":"Policy A",
                    "source_type":"policy", "supersedes":[]
                })))
                .unwrap();
        }
        NodeType::Resolution => {
            store
                .create_resolution(input::<CreateResolutionInput>(json!({
                    "scope_id":"default", "id":record_id, "title":"Storage",
                    "position":"Store records.", "rationale":"Records are required.",
                    "status":"draft", "requirement_ids":["requirement_a"],
                    "supersedes":[], "inputs":[]
                })))
                .unwrap();
        }
        NodeType::Rule => {
            store
                .create_rule(input::<CreateRuleInput>(json!({
                    "scope_id":"default", "id":record_id,
                    "statement":"The system stores each record.", "status":"draft",
                    "severity":"medium", "requirement_ids":["requirement_a"],
                    "resolution_ids":[]
                })))
                .unwrap();
        }
        NodeType::Domain => {
            store
                .create_domain(input::<CreateDomainInput>(json!({
                    "scope_id":"default", "id":record_id, "name":"Storage"
                })))
                .unwrap();
        }
        NodeType::Boundary => {
            store
                .create_boundary(input::<CreateBoundaryInput>(json!({
                    "scope_id":"default", "id":record_id,
                    "requirement_id":"requirement_a", "statement":"Storage only."
                })))
                .unwrap();
        }
        NodeType::Topic => {
            store
                .create_topic(input::<CreateTopicInput>(json!({
                    "scope_id":"default", "id":record_id,
                    "requirement_id":"requirement_a", "title":"Storage topic",
                    "status":"open", "links":[]
                })))
                .unwrap();
        }
        NodeType::Question => {
            store
                .create_question(input::<CreateQuestionInput>(json!({
                    "scope_id":"default", "id":record_id, "topic_id":"topic_parent",
                    "question":"Which storage?", "resolution_method":"research",
                    "status":"open", "links":[]
                })))
                .unwrap();
        }
        NodeType::Requirement => unreachable!("Requirement is the established review kind"),
    }
    record_id
}

fn revise_record(store: &StateStore, kind: NodeType, record_id: &StableId) {
    let common = json!({"scope_id":"default", "id":record_id});
    match kind {
        NodeType::Source => store
            .update_source(input::<UpdateSourceInput>(common_with(
                common,
                "name",
                json!("Policy Alpha"),
            )))
            .map(|_| ()),
        NodeType::Resolution => store
            .update_resolution(input::<UpdateResolutionInput>(common_with(
                common,
                "title",
                json!("Durable storage"),
            )))
            .map(|_| ()),
        NodeType::Rule => store
            .update_rule(input::<UpdateRuleInput>(common_with(
                common,
                "statement",
                json!("The system stores each durable record."),
            )))
            .map(|_| ()),
        NodeType::Domain => store
            .update_domain(input::<UpdateDomainInput>(common_with(
                common,
                "name",
                json!("Durable storage"),
            )))
            .map(|_| ()),
        NodeType::Boundary => store
            .update_boundary(input::<UpdateBoundaryInput>(common_with(
                common,
                "statement",
                json!("Durable storage only."),
            )))
            .map(|_| ()),
        NodeType::Topic => store
            .edit_topic(input::<UpdateTopicInput>(common_with(
                common,
                "title",
                json!("Durable storage topic"),
            )))
            .map(|_| ()),
        NodeType::Question => store
            .edit_question(input::<EditQuestionInput>(common_with(
                common,
                "question",
                json!("Which durable storage?"),
            )))
            .map(|_| ()),
        NodeType::Requirement => unreachable!("Requirement is the established review kind"),
    }
    .unwrap();
}

fn common_with(
    mut value: serde_json::Value,
    field: &str,
    content: serde_json::Value,
) -> serde_json::Value {
    value[field] = content;
    value
}

fn record_value(
    store: &StateStore,
    scope: &ScopeId,
    kind: NodeType,
    record_id: &StableId,
) -> serde_json::Value {
    macro_rules! find {
        ($reader:ident) => {
            serde_json::to_value(
                store
                    .$reader(scope)
                    .unwrap()
                    .into_iter()
                    .find(|record| record.id == *record_id)
                    .unwrap(),
            )
            .unwrap()
        };
    }
    match kind {
        NodeType::Source => find!(list_sources),
        NodeType::Resolution => find!(list_resolutions),
        NodeType::Rule => find!(list_rules),
        NodeType::Domain => find!(list_domains),
        NodeType::Boundary => find!(list_boundaries),
        NodeType::Topic => find!(list_topics),
        NodeType::Question => find!(list_questions),
        NodeType::Requirement => unreachable!("Requirement is the established review kind"),
    }
}

fn artifact_type(kind: NodeType) -> CanonicalArtifactType {
    serde_json::from_value(json!(kind.as_str())).unwrap()
}

fn review_feedback(kind: NodeType) -> Option<ReviewFeedback> {
    (!matches!(kind, NodeType::Domain | NodeType::Boundary)).then(|| ReviewFeedback {
        role: provenance_core::MessageRole::User,
        body: "Use the precise record text.".into(),
    })
}

fn run_review_cycle(kind: NodeType) {
    let (_temp, store, scope) = fixture();
    let record_id = create_record(&store, kind);
    let created = store
        .record_decision_state(&scope, kind, &record_id)
        .unwrap();
    assert!(created.pending.is_none());
    let first_revision = created.current_revision.unwrap();
    let first = store
        .submit_record_review(SubmitRecordReview {
            scope_id: scope.clone(),
            actor: "author".into(),
            record_kind: kind,
            record_id: record_id.clone(),
            declared_by: None,
            title: "Review Policy A".into(),
            summary: "Review the source definition.".into(),
            confidence: None,
            source_ids: Vec::new(),
            evidence_references: Vec::new(),
            builds_on: Vec::new(),
            expected_revision: Some(first_revision.clone()),
            revises: None,
        })
        .unwrap();
    assert!(!first.request_id.as_str().is_empty());
    let rejected = store
        .decide_record_review(DecideRecordReview {
            scope_id: scope.clone(),
            actor: reviewer(),
            proposal_id: first.proposal_id.clone(),
            decision: DispositionDecision::Rejected,
            rationale: Some("The source name is not precise.".into()),
            canonical_artifact: None,
            feedback: review_feedback(kind),
            declared_by: None,
        })
        .unwrap();
    assert!(!rejected.request_id.as_str().is_empty());
    assert!(rejected.disposition_id.is_some());

    revise_record(&store, kind, &record_id);
    let revised = store
        .record_decision_state(&scope, kind, &record_id)
        .unwrap();
    assert!(revised.pending.is_none());
    let second_revision = revised.current_revision.unwrap();
    let second = store
        .submit_record_review(SubmitRecordReview {
            scope_id: scope.clone(),
            actor: "author".into(),
            record_kind: kind,
            record_id: record_id.clone(),
            declared_by: None,
            title: "Review the revised record".into(),
            summary: "Review the precise record text.".into(),
            confidence: None,
            source_ids: Vec::new(),
            evidence_references: Vec::new(),
            builds_on: Vec::new(),
            expected_revision: Some(second_revision.clone()),
            revises: Some(first.proposal_id),
        })
        .unwrap();
    assert_ne!(second_revision, first_revision);

    let before_approval = record_value(&store, &scope, kind, &record_id);
    let approved = store
        .decide_record_review(DecideRecordReview {
            scope_id: scope.clone(),
            actor: reviewer(),
            proposal_id: second.proposal_id.clone(),
            decision: DispositionDecision::Accepted,
            rationale: None,
            canonical_artifact: Some(provenance_core::CanonicalArtifact {
                artifact_type: artifact_type(kind),
                artifact_id: record_id.clone(),
            }),
            feedback: None,
            declared_by: None,
        })
        .unwrap();
    assert!(!approved.request_id.as_str().is_empty());
    assert!(approved.disposition_id.is_some());
    assert_eq!(
        record_value(&store, &scope, kind, &record_id),
        before_approval
    );

    let decided = store
        .record_decision_state(&scope, kind, &record_id)
        .unwrap();
    assert_eq!(decided.decisions.len(), 2);
    assert_eq!(decided.decisions[0].revision, Some(first_revision));
    assert_eq!(decided.decisions[1].revision, Some(second_revision));
    assert_eq!(
        decided.current_acceptance.unwrap().disposition.proposal_id,
        second.proposal_id
    );
    assert!(decided.withdrawn.is_empty());
}

#[test]
fn every_added_kind_persists_the_exact_review_versions_without_lifecycle_change() {
    for kind in [
        NodeType::Source,
        NodeType::Resolution,
        NodeType::Rule,
        NodeType::Domain,
        NodeType::Boundary,
        NodeType::Topic,
        NodeType::Question,
    ] {
        run_review_cycle(kind);
    }
}

#[test]
fn domain_and_boundary_decisions_keep_rationale_and_refuse_feedback() {
    for kind in [NodeType::Domain, NodeType::Boundary] {
        let (_temp, store, scope) = fixture();
        let record_id = create_record(&store, kind);
        let current_revision = store
            .record_decision_state(&scope, kind, &record_id)
            .unwrap()
            .current_revision;
        let proposal = store
            .submit_record_review(SubmitRecordReview {
                scope_id: scope.clone(),
                actor: "author".into(),
                record_kind: kind,
                record_id: record_id.clone(),
                declared_by: None,
                title: "Review classification".into(),
                summary: "Review the classification.".into(),
                confidence: None,
                source_ids: Vec::new(),
                evidence_references: Vec::new(),
                builds_on: Vec::new(),
                expected_revision: current_revision,
                revises: None,
            })
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
                record_kind
            } if record_kind == kind
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
}
