use super::{root_of, seeded_store, sid};
use crate::operations::catalog::{
    self, ContextResolver, ExecutionNeeds, PreparedContext, PreparedRead, RequestedContext,
};
use crate::operations::{queries, read_policy::ReadPolicy};
use crate::state_store::{CreateResolutionInput, CreateRuleInput, StateStore};
use provenance_core::protocol::failure::OperationFailure;
use provenance_core::{
    ArchivedStamp, NodeType, ResolutionStatus, RuleSeverity, RuleStatus, ScopeId,
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
    catalog::invoke_with(
        "page-rules",
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
    assert!(list_rules(
        &root,
        json!({"exclude_terminal":false,"limit":1,"cursor":cursor}),
    )
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
            "exclude_terminal":false, "limit":1, "cursor":cursor
        }),
    )
    .await
    .unwrap_err()
    .to_string()
    .contains("cursor"));
}
