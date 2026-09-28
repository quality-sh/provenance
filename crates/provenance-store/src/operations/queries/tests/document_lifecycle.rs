use super::{root_of, seeded_store, sid};
use crate::operations::{queries, read_policy::ReadPolicy};
use crate::state_store::{
    CreateRequirementInput, CreateResolutionInput, CreateRuleInput, StateStore,
};
use provenance_core::protocol::{read_failure::ReadFailure, ReadDocumentQuery};
use provenance_core::{
    ArchivedStamp, RequirementStatus, ResolutionStatus, RuleSeverity, RuleStatus, ScopeId,
};
use serde_json::{json, Value};

fn add_requirement(store: &StateStore, scope: &ScopeId, id: &str, parent: &str) {
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: sid(id),
            statement: format!("{id} statement"),
            description: None,
            status: RequirementStatus::Active,
            domain_id: None,
            refines: Some(sid(parent)),
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
}

fn add_resolution(
    store: &StateStore,
    scope: &ScopeId,
    id: &str,
    requirement: &str,
    status: ResolutionStatus,
) {
    store
        .create_resolution(CreateResolutionInput {
            scope_id: scope.clone(),
            id: sid(id),
            title: format!("{id} title"),
            requirement_ids: vec![sid(requirement)],
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

fn add_rule(store: &StateStore, scope: &ScopeId, id: &str, requirement: &str, status: RuleStatus) {
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
            requirement_ids: vec![sid(requirement)],
            resolution_ids: Vec::new(),
            statement: format!("{id} statement"),
            status,
            severity: RuleSeverity::High,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
}

fn seed_lifecycle_records(store: &StateStore, scope: &ScopeId) {
    add_requirement(store, scope, "req_child", "req_overtime");
    add_requirement(store, scope, "req_nested", "req_child");
    add_resolution(
        store,
        scope,
        "res_abandoned_root",
        "req_overtime",
        ResolutionStatus::Abandoned,
    );
    add_resolution(
        store,
        scope,
        "res_abandoned_nested",
        "req_nested",
        ResolutionStatus::Abandoned,
    );
    add_resolution(
        store,
        scope,
        "res_superseded",
        "req_nested",
        ResolutionStatus::Superseded,
    );
    add_rule(
        store,
        scope,
        "rule_a_visible",
        "req_nested",
        RuleStatus::Active,
    );
    add_rule(
        store,
        scope,
        "rule_b_archived_root",
        "req_overtime",
        RuleStatus::Archived,
    );
    add_rule(
        store,
        scope,
        "rule_c_archived_nested",
        "req_nested",
        RuleStatus::Archived,
    );
    add_rule(
        store,
        scope,
        "rule_d_visible",
        "req_nested",
        RuleStatus::Active,
    );
}

async fn page(
    root: &camino::Utf8Path,
    cursor: Option<&str>,
    limit: usize,
    exclude_terminal: bool,
) -> anyhow::Result<Value> {
    let mut query = json!({
        "id": "req_overtime",
        "cursor": cursor,
        "limit": limit,
    });
    if exclude_terminal {
        query["exclude_terminal"] = json!(true);
    }
    let result = queries::read_document(
        Some(root.to_owned()),
        &ScopeId::new("default")?,
        ReadPolicy::default(),
        serde_json::from_value::<ReadDocumentQuery>(query)?,
    )
    .await?;
    Ok(serde_json::to_value(result.result)?)
}

async fn all_ids(root: &camino::Utf8Path, exclude_terminal: bool) -> Vec<String> {
    let mut cursor = None;
    let mut ids = Vec::new();
    loop {
        let answer = page(root, cursor.as_deref(), 50, exclude_terminal)
            .await
            .unwrap();
        ids.extend(
            answer["entries"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|entry| entry["node"]["id"].as_str().map(str::to_owned)),
        );
        cursor = answer["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            return ids;
        }
    }
}

#[tokio::test]
async fn lifecycle_filter_hides_dead_records_at_root_and_nested_levels() {
    let (dir, store, scope) = seeded_store();
    seed_lifecycle_records(&store, &scope);
    let root = root_of(&dir);

    let unfiltered = all_ids(&root, false).await;
    for id in [
        "res_abandoned_root",
        "res_abandoned_nested",
        "rule_b_archived_root",
        "rule_c_archived_nested",
        "res_superseded",
    ] {
        assert!(unfiltered.iter().any(|found| found == id), "missing {id}");
    }

    let filtered = all_ids(&root, true).await;
    for id in [
        "res_abandoned_root",
        "res_abandoned_nested",
        "rule_b_archived_root",
        "rule_c_archived_nested",
    ] {
        assert!(!filtered.iter().any(|found| found == id), "found {id}");
    }
    assert!(filtered.iter().any(|id| id == "res_superseded"));
}

#[tokio::test]
async fn lifecycle_filter_pages_across_hidden_records_and_binds_the_cursor() {
    let (dir, store, scope) = seeded_store();
    seed_lifecycle_records(&store, &scope);
    let root = root_of(&dir);
    let mut cursor = None;
    let mut pages = Vec::new();

    loop {
        let answer = page(&root, cursor.as_deref(), 1, true).await.unwrap();
        assert_eq!(answer["entries"].as_array().unwrap().len(), 1);
        pages.push(
            answer["entries"][0]["node"]["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
        cursor = answer["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            assert_eq!(answer["has_more"], false);
            break;
        }
        assert_eq!(answer["has_more"], true);
    }

    let visible_rule = pages.iter().position(|id| id == "rule_a_visible").unwrap();
    assert_eq!(pages[visible_rule + 1], "rule_d_visible");

    let first = page(&root, None, 1, true).await.unwrap();
    let filtered_cursor = first["next_cursor"].as_str().unwrap();
    let error = page(&root, Some(filtered_cursor), 1, false)
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::CursorInvalid)
    );
}
