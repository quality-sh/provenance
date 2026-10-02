use super::{root_of, seeded_store, sid};
use crate::operations::catalog::{
    self, ContextResolver, ExecutionNeeds, PreparedContext, PreparedRead, RequestedContext,
};
use crate::operations::{queries, read_policy::ReadPolicy};
use crate::review::SubmitRecordReview;
use crate::state_store::{
    CreateQuestionInput, CreateResolutionInput, CreateRuleInput, CreateTopicInput,
    EditQuestionInput, StateStore, UpdateTopicInput,
};
use provenance_core::protocol::failure::OperationFailure;
use provenance_core::{
    ArchivedStamp, NodeType, ResolutionMethod, ResolutionStatus, RuleSeverity, RuleStatus, ScopeId,
};
use serde_json::{json, Value};
use std::sync::Arc;

struct Target(camino::Utf8PathBuf);

impl ContextResolver for Target {
    fn prepare(
        &self,
        _: &'static str,
        _: RequestedContext,
        _: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        Ok(PreparedContext::read(PreparedRead {
            root: self.0.clone(),
            scope: ScopeId::new("default").unwrap(),
            policy: ReadPolicy::default(),
            requested_target: "test".into(),
            external: true,
        }))
    }
}

fn add_rule(store: &StateStore, scope: &ScopeId, id: &str, status: RuleStatus) {
    store
        .create_rule(CreateRuleInput {
            archived_in_commit: (status == RuleStatus::Archived).then(|| ArchivedStamp {
                commit: "a".repeat(40),
                at: None,
            }),
            scope_id: scope.clone(),
            id: sid(id),
            name: None,
            description: None,
            requirement_ids: vec![sid("req_overtime")],
            resolution_ids: Vec::new(),
            statement: "Shared lifecycle search term".into(),
            status,
            severity: RuleSeverity::High,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
}

fn add_resolution(store: &StateStore, scope: &ScopeId, id: &str, status: ResolutionStatus) {
    store
        .create_resolution(CreateResolutionInput {
            scope_id: scope.clone(),
            id: sid(id),
            title: "Shared lifecycle search term".into(),
            requirement_ids: vec![sid("req_overtime")],
            supersedes: Vec::new(),
            position: "Use the selected position".into(),
            rationale: "The evidence supports the position".into(),
            status,
            context: None,
            enforcement: None,
            confidence: None,
            inputs: Vec::new(),
            made_by: None,
            approved_by: None,
            approved_at: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
}

fn seed_lifecycle_records(store: &StateStore, scope: &ScopeId) {
    add_rule(store, scope, "rule_a_archived", RuleStatus::Archived);
    add_rule(store, scope, "rule_b_active", RuleStatus::Active);
    add_rule(store, scope, "rule_c_active", RuleStatus::Active);
    add_resolution(store, scope, "res_a_abandoned", ResolutionStatus::Abandoned);
    add_resolution(
        store,
        scope,
        "res_b_superseded",
        ResolutionStatus::Superseded,
    );
    add_resolution(store, scope, "res_c_approved", ResolutionStatus::Approved);
}

async fn list_rules(root: &camino::Utf8Path, request: Value) -> anyhow::Result<Value> {
    list_family(root, "page-rules", request).await
}

async fn list_family(
    root: &camino::Utf8Path,
    operation: &'static str,
    request: Value,
) -> anyhow::Result<Value> {
    catalog::invoke_with(
        operation,
        provenance_core::SDK_PROTOCOL_VERSION,
        json!({
            "context":{"repository":"test","scope":"default"},
            "request": request,
        }),
        Arc::new(Target(root.to_owned())),
    )
    .await
    .map_err(|error| anyhow::anyhow!(serde_json::to_string(&error).unwrap()))
}

fn seed_archived_shaping_records(store: &StateStore, scope: &ScopeId) {
    store
        .create_topic(CreateTopicInput {
            scope_id: scope.clone(),
            id: sid("topic_archived"),
            requirement_id: sid("req_overtime"),
            title: "Archived lifecycle search term".into(),
            status: serde_json::from_str("\"open\"").unwrap(),
            links: Vec::new(),
        })
        .unwrap();
    for id in ["question_archived_one", "question_archived_two"] {
        store
            .create_question(CreateQuestionInput {
                scope_id: scope.clone(),
                id: sid(id),
                topic_id: sid("topic_archived"),
                question: "Archived lifecycle search term?".into(),
                resolution_method: ResolutionMethod::Research,
                status: serde_json::from_str("\"open\"").unwrap(),
                answer: None,
                links: Vec::new(),
                resolution_id: None,
                contradicts: None,
            })
            .unwrap();
    }
    for (kind, id) in [
        (NodeType::Topic, "topic_archived"),
        (NodeType::Question, "question_archived_one"),
        (NodeType::Question, "question_archived_two"),
    ] {
        let id = sid(id);
        let revision = store
            .record_decision_state(scope, kind, &id)
            .unwrap()
            .current_revision
            .unwrap();
        store
            .submit_record_review(SubmitRecordReview {
                scope_id: scope.clone(),
                actor: "author".into(),
                record_kind: kind,
                record_id: id,
                declared_by: None,
                title: "Review archive candidate".into(),
                summary: "Review this shaping record.".into(),
                confidence: None,
                source_ids: Vec::new(),
                evidence_references: Vec::new(),
                builds_on: Vec::new(),
                expected_revision: Some(revision),
                revises: None,
            })
            .unwrap();
    }
    store
        .edit_topic(
            serde_json::from_value::<UpdateTopicInput>(json!({
                "scope_id":scope, "id":"topic_archived", "status":"archived",
                "archived_in_commit":{"commit":"b".repeat(40)}
            }))
            .unwrap(),
        )
        .unwrap();
}

async fn search(root: &camino::Utf8Path, request: Value) -> anyhow::Result<Value> {
    let result = queries::search(
        Some(root.to_owned()),
        &ScopeId::new("default")?,
        ReadPolicy::default(),
        serde_json::from_value(request)?,
    )
    .await?;
    Ok(serde_json::to_value(result.result)?)
}

#[tokio::test]
async fn lifecycle_filter_hides_only_terminal_records_from_lists_and_search() {
    let (dir, store, scope) = seeded_store();
    seed_lifecycle_records(&store, &scope);
    let root = root_of(&dir);

    let rules = list_rules(&root, json!({"exclude_terminal":true,"limit":50}))
        .await
        .unwrap();
    let listed = serde_json::to_string(&rules["result"]["items"]).unwrap();
    assert!(!listed.contains("rule_a_archived"));
    assert!(listed.contains("rule_b_active"));

    let found = search(
        &root,
        json!({"text":"lifecycle search", "exclude_terminal":true, "limit":50}),
    )
    .await
    .unwrap();
    let found = serde_json::to_string(&found["nodes"]).unwrap();
    assert!(!found.contains("rule_a_archived"));
    assert!(!found.contains("res_a_abandoned"));
    assert!(found.contains("rule_b_active"));
    assert!(found.contains("res_b_superseded"));
    assert!(found.contains("res_c_approved"));
}

#[tokio::test]
async fn lifecycle_filter_precedes_page_counts_and_binds_each_cursor() {
    let (dir, store, scope) = seeded_store();
    seed_lifecycle_records(&store, &scope);
    let root = root_of(&dir);

    let first = list_rules(&root, json!({"exclude_terminal":true,"limit":1}))
        .await
        .unwrap();
    assert_eq!(first["result"]["items"][0]["id"], "rule_b_active");
    assert_eq!(first["result"]["has_more"], true);
    let cursor = first["result"]["next_cursor"].clone();
    let second = list_rules(
        &root,
        json!({"exclude_terminal":true,"limit":1,"cursor":cursor}),
    )
    .await
    .unwrap();
    assert_eq!(second["result"]["items"][0]["id"], "rule_c_active");
    assert_eq!(second["result"]["has_more"], false);
    assert!(list_rules(&root, json!({"limit":1,"cursor":cursor}))
        .await
        .unwrap_err()
        .to_string()
        .contains("cursor"));

    let first = search(
        &root,
        json!({
            "text":"lifecycle search", "node_types":[NodeType::Rule],
            "exclude_terminal":true, "limit":1
        }),
    )
    .await
    .unwrap();
    assert_eq!(first["nodes"][0]["id"], "rule_b_active");
    assert_eq!(first["has_more"], true);
    let cursor = first["next_cursor"].clone();
    let second = search(
        &root,
        json!({
            "text":"lifecycle search", "node_types":[NodeType::Rule],
            "exclude_terminal":true, "limit":1, "cursor":cursor
        }),
    )
    .await
    .unwrap();
    assert_eq!(second["nodes"][0]["id"], "rule_c_active");
    assert_eq!(second["has_more"], false);
    assert!(search(
        &root,
        json!({
            "text":"lifecycle search", "node_types":[NodeType::Rule],
            "limit":1, "cursor":cursor
        }),
    )
    .await
    .unwrap_err()
    .to_string()
    .contains("cursor"));
}

#[tokio::test]
async fn archived_topics_and_questions_are_hidden_from_filtered_reads_and_totals() {
    let (dir, store, scope) = seeded_store();
    seed_archived_shaping_records(&store, &scope);
    let root = root_of(&dir);

    for operation in ["page-topics", "page-questions"] {
        let unfiltered = list_family(&root, operation, json!({"limit":50}))
            .await
            .unwrap();
        assert_ne!(
            unfiltered["result"]["items"].as_array().unwrap().as_slice(),
            &[] as &[Value]
        );
        let filtered = list_family(
            &root,
            operation,
            json!({"exclude_terminal":true,"limit":50}),
        )
        .await
        .unwrap();
        assert_eq!(
            filtered["result"]["items"].as_array().unwrap().as_slice(),
            &[] as &[Value]
        );
    }

    let found = search(
        &root,
        json!({"text":"archived lifecycle", "exclude_terminal":true, "limit":50}),
    )
    .await
    .unwrap();
    assert_eq!(
        found["nodes"].as_array().unwrap().as_slice(),
        &[] as &[Value]
    );

    let unfiltered_document = queries::read_document(
        Some(root.clone()),
        &scope,
        ReadPolicy::default(),
        provenance_core::protocol::ReadDocumentQuery {
            id: "req_overtime".into(),
            exclude_terminal: false,
            cursor: None,
            limit: 50,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(unfiltered_document.result).unwrap()["review_totals"],
        json!({"pending":4,"accepted":0,"rejected":0})
    );

    let document = queries::read_document(
        Some(root),
        &scope,
        ReadPolicy::default(),
        provenance_core::protocol::ReadDocumentQuery {
            id: "req_overtime".into(),
            exclude_terminal: true,
            cursor: None,
            limit: 50,
        },
    )
    .await
    .unwrap();
    let document = serde_json::to_value(document.result).unwrap();
    assert!(document["entries"].as_array().unwrap().iter().all(|entry| {
        !matches!(
            entry["node"]["node_type"].as_str(),
            Some("topic" | "question")
        )
    }));
    assert_eq!(
        document["review_totals"],
        json!({"pending":1,"accepted":0,"rejected":0})
    );
}

#[test]
fn archived_question_cannot_return_to_an_active_status() {
    let (_dir, store, scope) = seeded_store();
    seed_archived_shaping_records(&store, &scope);
    let input = serde_json::from_value::<EditQuestionInput>(json!({
        "scope_id":scope, "id":"question_archived_one", "status":"open"
    }))
    .unwrap();
    assert!(store
        .edit_question(input)
        .unwrap_err()
        .to_string()
        .contains("terminal"));
}
