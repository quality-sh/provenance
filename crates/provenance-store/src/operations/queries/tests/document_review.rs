use super::{root_of, seeded_store, sid};
use crate::operations::{queries, read_policy::ReadPolicy};
use crate::review::{DecideRecordReview, ReviewFeedback, SubmitRecordReview};
use crate::state_store::{
    AddSourceReferenceInput, CreateQuestionInput, CreateRequirementInput, CreateResolutionInput,
    CreateRuleInput, CreateTopicInput, EditQuestionInput, StateStore, UpdateBoundaryInput,
    UpdateDomainInput, UpdateRequirementInput, UpdateResolutionInput, UpdateRuleInput,
    UpdateSourceInput, UpdateTopicInput,
};
use provenance_core::protocol::ReadDocumentQuery;
use provenance_core::{
    CanonicalArtifact, CanonicalArtifactType, DispositionActor, DispositionDecision, IdentityType,
    NodeType, ScopeId, StableId,
};
use serde_json::{json, Value};

pub(super) fn input<T: serde::de::DeserializeOwned>(value: Value) -> T {
    serde_json::from_value(value).unwrap()
}

fn allow_reviewer(store: &StateStore) {
    std::fs::write(
        store.layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}],"disposition_actor_ids":["reviewer"]}"#,
    )
    .unwrap();
}

async fn page(root: &camino::Utf8Path, limit: usize, exclude_terminal: bool) -> Value {
    let result = queries::read_document(
        Some(root.to_owned()),
        &ScopeId::new("default").unwrap(),
        ReadPolicy::default(),
        ReadDocumentQuery {
            id: "req_overtime".into(),
            exclude_terminal,
            cursor: None,
            limit,
        },
    )
    .await
    .unwrap();
    serde_json::to_value(result.result).unwrap()
}

fn review<'a>(page: &'a Value, id: &str) -> &'a Value {
    &page["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["node"]["id"] == id)
        .unwrap()["review"]
}

fn create_document_records(store: &StateStore, scope: &ScopeId) -> Vec<(NodeType, StableId)> {
    store
        .create_source(input(json!({
            "scope_id":scope, "id":"source_policy", "name":"Policy",
            "source_type":"policy", "supersedes":[]
        })))
        .unwrap();
    store
        .add_source_reference(AddSourceReferenceInput {
            scope_id: scope.clone(),
            source_id: sid("source_policy"),
            requirement_id: sid("req_overtime"),
            clause: None,
        })
        .unwrap();
    store
        .create_resolution(input::<CreateResolutionInput>(json!({
            "scope_id":scope, "id":"resolution_policy", "title":"Policy decision",
            "position":"Use the policy.", "rationale":"The policy is current.",
            "status":"draft", "requirement_ids":["req_overtime"], "supersedes":[], "inputs":[]
        })))
        .unwrap();
    store
        .create_rule(input::<CreateRuleInput>(json!({
            "scope_id":scope, "id":"rule_policy", "statement":"The system uses the policy.",
            "status":"draft", "severity":"medium", "requirement_ids":["req_overtime"],
            "resolution_ids":[]
        })))
        .unwrap();
    store
        .create_topic(input::<CreateTopicInput>(json!({
            "scope_id":scope, "id":"topic_policy", "requirement_id":"req_overtime",
            "title":"Policy topic", "status":"open", "links":[]
        })))
        .unwrap();
    store
        .create_question(input::<CreateQuestionInput>(json!({
            "scope_id":scope, "id":"question_policy", "topic_id":"topic_policy",
            "question":"Which policy?", "resolution_method":"research",
            "status":"open", "links":[]
        })))
        .unwrap();
    vec![
        (NodeType::Source, sid("source_policy")),
        (NodeType::Requirement, sid("req_overtime")),
        (NodeType::Resolution, sid("resolution_policy")),
        (NodeType::Rule, sid("rule_policy")),
        (NodeType::Domain, sid("domain_payroll")),
        (NodeType::Boundary, sid("boundary_no_backpay")),
        (NodeType::Topic, sid("topic_policy")),
        (NodeType::Question, sid("question_policy")),
    ]
}

fn submit(store: &StateStore, scope: &ScopeId, kind: NodeType, id: &StableId) -> StableId {
    let revision = store
        .record_decision_state(scope, kind, id)
        .unwrap()
        .current_revision
        .unwrap();
    store
        .submit_record_review(SubmitRecordReview {
            scope_id: scope.clone(),
            actor: "author".into(),
            record_kind: kind,
            record_id: id.clone(),
            declared_by: None,
            title: "Review the record".into(),
            summary: "Review the current record text.".into(),
            confidence: None,
            source_ids: Vec::new(),
            evidence_references: Vec::new(),
            builds_on: Vec::new(),
            expected_revision: Some(revision),
            revises: None,
        })
        .unwrap()
        .proposal_id
}

fn decide(
    store: &StateStore,
    scope: &ScopeId,
    kind: NodeType,
    id: &StableId,
    proposal: StableId,
    decision: DispositionDecision,
) {
    let feedback = (decision == DispositionDecision::Rejected
        && !matches!(kind, NodeType::Domain | NodeType::Boundary))
    .then(|| ReviewFeedback {
        role: provenance_core::MessageRole::User,
        body: "Use more precise text.".into(),
    });
    store
        .decide_record_review(DecideRecordReview {
            scope_id: scope.clone(),
            actor: DispositionActor {
                identity_type: IdentityType::Human,
                id: "reviewer".into(),
                name: None,
            },
            proposal_id: proposal,
            decision,
            rationale: (decision == DispositionDecision::Rejected)
                .then(|| "The text is not precise.".into()),
            canonical_artifact: (decision == DispositionDecision::Accepted).then(|| {
                CanonicalArtifact {
                    artifact_type: input::<CanonicalArtifactType>(json!(kind.as_str())),
                    artifact_id: id.clone(),
                }
            }),
            feedback,
            declared_by: None,
        })
        .unwrap();
}

fn revise(store: &StateStore, kind: NodeType, id: &StableId) {
    let common = json!({"scope_id":"default", "id":id});
    match kind {
        NodeType::Source => store
            .update_source(input::<UpdateSourceInput>(with(
                common,
                "name",
                "Policy revised",
            )))
            .map(|_| ()),
        NodeType::Requirement => store
            .update_requirement(input::<UpdateRequirementInput>(with(
                common,
                "statement",
                "Overtime is always paid",
            )))
            .map(|_| ()),
        NodeType::Resolution => store
            .update_resolution(input::<UpdateResolutionInput>(with(
                common,
                "title",
                "Revised policy decision",
            )))
            .map(|_| ()),
        NodeType::Rule => store
            .update_rule(input::<UpdateRuleInput>(with(
                common,
                "statement",
                "The system always uses the policy.",
            )))
            .map(|_| ()),
        NodeType::Domain => store
            .update_domain(input::<UpdateDomainInput>(with(
                common,
                "name",
                "Payroll policy",
            )))
            .map(|_| ()),
        NodeType::Boundary => store
            .update_boundary(input::<UpdateBoundaryInput>(with(
                common,
                "statement",
                "Historical back pay is out of scope.",
            )))
            .map(|_| ()),
        NodeType::Topic => store
            .edit_topic(input::<UpdateTopicInput>(with(
                common,
                "title",
                "Revised policy topic",
            )))
            .map(|_| ()),
        NodeType::Question => store
            .edit_question(input::<EditQuestionInput>(with(
                common,
                "question",
                "Which current policy?",
            )))
            .map(|_| ()),
    }
    .unwrap();
}

fn with(mut value: Value, field: &str, content: &str) -> Value {
    value[field] = json!(content);
    value
}

#[tokio::test]
#[provenance_macros::verifies("rule_document_entry_has_review_outcome", examples)]
async fn document_reports_each_reviewable_kind_through_the_full_decision_cycle() {
    let (dir, store, scope) = seeded_store();
    allow_reviewer(&store);
    let records = create_document_records(&store, &scope);
    let root = root_of(&dir);

    for (kind, id) in records {
        let initial = page(&root, 50, false).await;
        let first = if kind == NodeType::Requirement {
            assert_eq!(review(&initial, id.as_str())["outcome"], "pending");
            store
                .record_decision_state(&scope, kind, &id)
                .unwrap()
                .pending
                .unwrap()
                .proposal_id
        } else {
            assert_eq!(review(&initial, id.as_str())["outcome"], Value::Null);
            submit(&store, &scope, kind, &id)
        };
        let pending = page(&root, 50, false).await;
        assert_eq!(review(&pending, id.as_str())["outcome"], "pending");
        assert_eq!(
            review(&pending, id.as_str())["pending_proposal_id"],
            first.as_str()
        );

        decide(
            &store,
            &scope,
            kind,
            &id,
            first,
            DispositionDecision::Accepted,
        );
        let accepted = page(&root, 50, false).await;
        assert_eq!(review(&accepted, id.as_str())["outcome"], "accepted");
        assert_eq!(review(&accepted, id.as_str())["comment_count"], 0);

        revise(&store, kind, &id);
        let revised = page(&root, 50, false).await;
        let second = if kind == NodeType::Requirement {
            assert_eq!(review(&revised, id.as_str())["outcome"], "pending");
            store
                .record_decision_state(&scope, kind, &id)
                .unwrap()
                .pending
                .unwrap()
                .proposal_id
        } else {
            assert_eq!(review(&revised, id.as_str())["outcome"], Value::Null);
            submit(&store, &scope, kind, &id)
        };
        decide(
            &store,
            &scope,
            kind,
            &id,
            second,
            DispositionDecision::Rejected,
        );
        let rejected = page(&root, 50, false).await;
        assert_eq!(review(&rejected, id.as_str())["outcome"], "rejected");
        let expected_comments = usize::from(!matches!(kind, NodeType::Domain | NodeType::Boundary));
        assert_eq!(
            review(&rejected, id.as_str())["comment_count"],
            expected_comments
        );
    }
}

#[tokio::test]
#[provenance_macros::verifies("rule_document_has_review_totals", examples)]
/// This test covers review totals through filtered document pagination.
async fn document_review_totals_follow_the_filter_and_repeat_on_each_page() {
    let (dir, store, scope) = seeded_store();
    allow_reviewer(&store);
    create_document_records(&store, &scope);
    let pending = store
        .record_decision_state(&scope, NodeType::Requirement, &sid("req_overtime"))
        .unwrap()
        .pending
        .unwrap()
        .proposal_id;
    assert_ne!(pending.as_str(), "");
    let accepted = submit(
        &store,
        &scope,
        NodeType::Resolution,
        &sid("resolution_policy"),
    );
    decide(
        &store,
        &scope,
        NodeType::Resolution,
        &sid("resolution_policy"),
        accepted,
        DispositionDecision::Accepted,
    );
    let rejected = submit(&store, &scope, NodeType::Rule, &sid("rule_policy"));
    decide(
        &store,
        &scope,
        NodeType::Rule,
        &sid("rule_policy"),
        rejected,
        DispositionDecision::Rejected,
    );
    store
        .update_rule(input::<UpdateRuleInput>(json!({
            "scope_id":"default", "id":"rule_policy", "status":"archived",
            "archived_in_commit":{"commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}
        })))
        .unwrap();

    let root = root_of(&dir);
    assert_eq!(
        page(&root, 50, false).await["review_totals"],
        json!({
            "pending":1, "accepted":1, "rejected":1
        })
    );
    let filtered = page(&root, 1, true).await;
    assert_eq!(
        filtered["review_totals"],
        json!({
            "pending":1, "accepted":1, "rejected":0
        })
    );
    let cursor = filtered["next_cursor"].as_str().unwrap();
    let next = queries::read_document(
        Some(root),
        &scope,
        ReadPolicy::default(),
        ReadDocumentQuery {
            id: "req_overtime".into(),
            exclude_terminal: true,
            cursor: Some(cursor.into()),
            limit: 1,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(next.result).unwrap()["review_totals"],
        filtered["review_totals"]
    );
}

#[tokio::test]
async fn document_review_totals_exclude_a_pending_parent_reference() {
    let (dir, store, scope) = seeded_store();
    allow_reviewer(&store);
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: sid("req_first_release"),
            statement: "The first release includes overtime.".into(),
            description: None,
            status: provenance_core::RequirementStatus::Active,
            domain_id: None,
            refines: Some(sid("req_overtime")),
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    let root = root_of(&dir);
    let read = |cursor| {
        queries::read_document(
            Some(root.clone()),
            &scope,
            ReadPolicy::default(),
            ReadDocumentQuery {
                id: "req_first_release".into(),
                exclude_terminal: true,
                cursor,
                limit: 50,
            },
        )
    };

    let pending = serde_json::to_value(read(None).await.unwrap().result).unwrap();
    assert_eq!(pending["entries"][0]["kind"], "member");
    assert_eq!(pending["entries"][0]["node"]["id"], "req_first_release");
    assert_eq!(pending["entries"][0]["review"]["outcome"], "pending");
    assert_eq!(pending["entries"][1]["kind"], "reference");
    assert_eq!(pending["entries"][1]["node"]["id"], "req_overtime");
    assert_eq!(pending["entries"][1]["review"]["outcome"], "pending");
    assert_eq!(
        pending["review_totals"],
        json!({"pending":1, "accepted":0, "rejected":0})
    );

    let proposal = store
        .record_decision_state(&scope, NodeType::Requirement, &sid("req_first_release"))
        .unwrap()
        .pending
        .unwrap()
        .proposal_id;
    decide(
        &store,
        &scope,
        NodeType::Requirement,
        &sid("req_first_release"),
        proposal,
        DispositionDecision::Rejected,
    );
    let rejected = serde_json::to_value(read(None).await.unwrap().result).unwrap();
    assert_eq!(
        rejected["review_totals"],
        json!({"pending":0, "accepted":0, "rejected":1})
    );
}
