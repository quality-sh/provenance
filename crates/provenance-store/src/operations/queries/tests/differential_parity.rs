//! The differential gate: old executor versus served executor.
//!
//! For every operation the pre-re-back executor and the served executor
//! run over the same fixture, and the serialized answers must match
//! except for the additive fields the re-back introduced (the freshness
//! stamp and continuation cursors). This runs in CI permanently as the
//! drift alarm for the served read path.

use super::originals::{
    evidence as legacy_evidence, impact as legacy_impact, records as legacy_records,
    symbols as legacy_symbols, walk as legacy_walk,
};
use serde_json::json;

use crate::operations::queries;
use crate::state_store::StateStore;
use provenance_core::protocol::{
    EvidenceQuery, ImpactQuery, NeighborsQuery, ResolveSymbolQuery, SearchQuery, TraceQuery,
};
use provenance_core::{ScopeId, StableId};

fn strip_additive(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.remove("stamp");
            map.remove("next_cursor");
            for key in [
                "implementation_bindings_page",
                "verification_bindings_page",
                "verification_runs_page",
                "reviews_page",
            ] {
                map.remove(key);
            }
            for (_, child) in map.iter_mut() {
                strip_additive(child);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                strip_additive(item);
            }
        }
        _ => {}
    }
}

fn canonical(old: &impl serde::Serialize, new: &impl serde::Serialize) -> (String, String) {
    let mut old_value = serde_json::to_value(old).unwrap();
    let mut new_value = serde_json::to_value(new).unwrap();
    strip_additive(&mut old_value);
    strip_additive(&mut new_value);
    (
        serde_json::to_string(&old_value).unwrap(),
        serde_json::to_string(&new_value).unwrap(),
    )
}

fn seeded() -> (tempfile::TempDir, crate::layout::ProvenanceLayout, ScopeId) {
    let (dir, layout, scope) = crate::cache::tests::fixtures::seeded_layout();
    let store = StateStore::new(layout.clone());
    for requirement in ["req_breadth_one", "req_breadth_two"] {
        crate::cache::tests::fixtures::create_requirement(
            &store,
            &scope,
            requirement,
            provenance_core::RequirementStatus::Active,
        );
    }
    for requirement in ["req_breadth_one", "req_breadth_two"] {
        store
            .add_source_reference(crate::state_store::AddSourceReferenceInput {
                scope_id: scope.clone(),
                source_id: StableId::new("source_schads").unwrap(),
                requirement_id: StableId::new(requirement).unwrap(),
                clause: None,
            })
            .unwrap();
    }
    (dir, layout, scope)
}

fn assert_same_bytes(old: impl serde::Serialize, new: impl serde::Serialize) {
    let (old_json, new_json) = canonical(&old, &new);
    assert_eq!(old_json, new_json);
}

fn get_request(id: &str, node_type: &str) -> serde_json::Value {
    serde_json::json!({ "node_type": node_type, "id": id })
}

#[tokio::test]
async fn served_get_matches_the_original_executor() {
    let (_dir, layout, scope) = seeded();
    crate::cache::materialize_state(&layout).await.unwrap();
    let repo = Some(layout.root().to_path_buf());
    let store = StateStore::new(layout.clone());
    let old = legacy_records::get(
        &store,
        &scope,
        serde_json::from_value(get_request("req_schads_overtime", "requirement")).unwrap(),
    )
    .unwrap();
    let new = queries::get(
        repo,
        &scope,
        serde_json::from_value(get_request("req_schads_overtime", "requirement")).unwrap(),
    )
    .await
    .unwrap();
    assert_same_bytes(old, new);
}

#[tokio::test]
async fn served_search_matches_the_original_executor() {
    let (_dir, layout, scope) = seeded();
    crate::cache::materialize_state(&layout).await.unwrap();
    let repo = Some(layout.root().to_path_buf());
    let store = StateStore::new(layout.clone());
    let old = legacy_records::search(
        &store,
        &scope,
        serde_json::from_value::<SearchQuery>(serde_json::json!({
            "text": "overtime", "limit": 50
        }))
        .unwrap(),
    )
    .unwrap();
    let new = queries::search(
        repo,
        &scope,
        serde_json::from_value::<SearchQuery>(serde_json::json!({
            "text": "overtime", "limit": 50
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    assert_same_bytes(old, new);
}

#[tokio::test]
async fn served_neighbors_matches_the_original_executor() {
    let (_dir, layout, scope) = seeded();
    crate::cache::materialize_state(&layout).await.unwrap();
    let repo = Some(layout.root().to_path_buf());
    let store = StateStore::new(layout.clone());
    let old = legacy_walk::neighbors(
        &store,
        &scope,
        serde_json::from_value::<NeighborsQuery>(serde_json::json!({
            "id": "source_schads", "direction": "both", "limit": 50
        }))
        .unwrap(),
    )
    .unwrap();
    let new = queries::neighbors(
        repo,
        &scope,
        serde_json::from_value::<NeighborsQuery>(serde_json::json!({
            "id": "source_schads", "direction": "both", "limit": 50
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    assert_same_bytes(old, new);
}

#[tokio::test]
async fn served_trace_matches_the_original_executor() {
    let (_dir, layout, scope) = seeded();
    crate::cache::materialize_state(&layout).await.unwrap();
    let repo = Some(layout.root().to_path_buf());
    let store = StateStore::new(layout.clone());
    let old = legacy_walk::trace(
        &store,
        &scope,
        serde_json::from_value::<TraceQuery>(serde_json::json!({
            "id": "source_schads", "direction": "both", "limit": 200
        }))
        .unwrap(),
    )
    .unwrap();
    let new = queries::trace(
        repo,
        &scope,
        serde_json::from_value::<TraceQuery>(serde_json::json!({
            "id": "source_schads", "direction": "both", "limit": 200
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    assert_same_bytes(old, new);
}

#[tokio::test]
async fn served_impact_matches_the_original_executor() {
    let (_dir, layout, scope) = seeded();
    crate::cache::materialize_state(&layout).await.unwrap();
    let repo = Some(layout.root().to_path_buf());
    let store = StateStore::new(layout.clone());
    let old = legacy_impact::impact(
        layout.root(),
        &store,
        &scope,
        serde_json::from_value::<ImpactQuery>(serde_json::json!({
            "id": "source_schads", "limit": 200
        }))
        .unwrap(),
    )
    .unwrap();
    let new = queries::impact(
        repo,
        &scope,
        serde_json::from_value::<ImpactQuery>(serde_json::json!({
            "id": "source_schads", "limit": 200
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    assert_same_bytes(old, new);
}

#[tokio::test]
async fn served_resolve_symbol_matches_the_original_executor() {
    let (_dir, layout, scope) = seeded();
    crate::cache::materialize_state(&layout).await.unwrap();
    let repo = Some(layout.root().to_path_buf());
    let store = StateStore::new(layout.clone());
    let old = legacy_symbols::resolve(
        layout.root(),
        &store,
        &scope,
        serde_json::from_value::<ResolveSymbolQuery>(serde_json::json!({
            "file": "src/lib.rs", "limit": 200
        }))
        .unwrap(),
    )
    .unwrap();
    let new = queries::resolve_symbol(
        repo,
        &scope,
        serde_json::from_value::<ResolveSymbolQuery>(serde_json::json!({
            "file": "src/lib.rs", "limit": 200
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    assert_same_bytes(old, new);
}

#[tokio::test]
async fn served_evidence_matches_the_original_executor() {
    let (_dir, layout, scope) = seeded();
    crate::cache::materialize_state(&layout).await.unwrap();
    let repo = Some(layout.root().to_path_buf());
    let store = StateStore::new(layout.clone());

    let old = legacy_evidence::evidence(
        layout.root(),
        &store,
        &scope,
        serde_json::from_value::<EvidenceQuery>(serde_json::json!({
            "rule": "rule_schads_pay_001", "limit": 50
        }))
        .unwrap(),
    )
    .unwrap();
    let new = queries::evidence(
        repo,
        &scope,
        serde_json::from_value::<EvidenceQuery>(serde_json::json!({
            "rule": "rule_schads_pay_001", "limit": 50
        }))
        .unwrap(),
    )
    .await
    .unwrap();

    let (old_json, new_json) = canonical(&old, &new);
    assert_eq!(old_json, new_json);
    assert!(new.stamp.is_some(), "served evidence carries the stamp");
    assert!(
        !new.implementation_bindings_page.has_more,
        "empty collections page truthfully"
    );
    let _ = StableId::new("rule_schads_pay_001").unwrap();
}

#[tokio::test]
async fn search_and_neighbors_pages_exhaust_to_the_unpaginated_answer() {
    let (_dir, layout, scope) = seeded();
    crate::cache::materialize_state(&layout).await.unwrap();
    let repo = Some(layout.root().to_path_buf());

    let whole = queries::search(
        repo.clone(),
        &scope,
        serde_json::from_value::<SearchQuery>(serde_json::json!({
            "text": "e", "limit": 200
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    let mut paged_ids = Vec::new();
    let mut cursor = None;
    loop {
        let mut input = serde_json::json!({"text": "e", "limit": 1});
        if let Some(token) = &cursor {
            input["cursor"] = json!(token);
        }
        let page: provenance_core::protocol::SearchResult =
            queries::search(repo.clone(), &scope, serde_json::from_value(input).unwrap())
                .await
                .unwrap();
        paged_ids.extend(page.nodes.iter().map(|node| node.id().as_str().to_string()));
        match &page.next_cursor {
            Some(token) => cursor = Some(token.clone()),
            None => break,
        }
        assert!(
            paged_ids.len() <= whole.nodes.len() + 1,
            "exhaustion terminates"
        );
    }
    let whole_ids: Vec<String> = whole
        .nodes
        .iter()
        .map(|node| node.id().as_str().to_string())
        .collect();
    assert_eq!(paged_ids, whole_ids, "pages concatenate to the full answer");
}

#[tokio::test]
async fn trace_resume_equals_the_untruncated_walk() {
    let (_dir, layout, scope) = seeded();
    crate::cache::materialize_state(&layout).await.unwrap();
    let repo = Some(layout.root().to_path_buf());

    let untruncated = queries::trace(
        repo.clone(),
        &scope,
        serde_json::from_value::<TraceQuery>(serde_json::json!({
            "id": "source_schads", "direction": "both", "max_depth": 5, "limit": 200
        }))
        .unwrap(),
    )
    .await
    .unwrap();

    // First page truncates at one node mid-breadth.
    let first = queries::trace(
        repo.clone(),
        &scope,
        serde_json::from_value::<TraceQuery>(serde_json::json!({
            "id": "source_schads", "direction": "both", "max_depth": 5, "limit": 1
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    assert!(first.has_more);
    let token = first
        .next_cursor
        .clone()
        .expect("truncated page carries a token");

    let resumed = queries::trace(
        repo,
        &scope,
        serde_json::from_value::<TraceQuery>(serde_json::json!({
            "id": "source_schads", "direction": "both", "max_depth": 5,
            "limit": 200, "cursor": token
        }))
        .unwrap(),
    )
    .await
    .unwrap();

    let mut combined: Vec<(usize, String)> = first
        .nodes
        .iter()
        .map(|traced| (traced.depth, traced.node.id().as_str().to_string()))
        .collect();
    combined.extend(
        resumed
            .nodes
            .iter()
            .map(|traced| (traced.depth, traced.node.id().as_str().to_string())),
    );
    let expected: Vec<(usize, String)> = untruncated
        .nodes
        .iter()
        .map(|traced| (traced.depth, traced.node.id().as_str().to_string()))
        .collect();
    assert_eq!(combined, expected, "resume reproduces the untruncated walk");
}

#[tokio::test]
async fn trace_resume_rejects_a_token_from_a_different_request() {
    let (_dir, layout, scope) = seeded();
    crate::cache::materialize_state(&layout).await.unwrap();
    let repo = Some(layout.root().to_path_buf());

    let first = queries::trace(
        repo.clone(),
        &scope,
        serde_json::from_value::<TraceQuery>(serde_json::json!({
            "id": "source_schads", "direction": "both", "max_depth": 5, "limit": 1
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    let token = first.next_cursor.expect("truncated page carries a token");

    let mismatched = queries::trace(
        repo,
        &scope,
        serde_json::from_value::<TraceQuery>(serde_json::json!({
            "id": "source_schads", "direction": "out", "max_depth": 5,
            "limit": 200, "cursor": token
        }))
        .unwrap(),
    )
    .await;
    assert!(
        mismatched.is_err(),
        "a token must not survive a parameter change"
    );
}

#[tokio::test]
async fn visit_budget_stops_the_walk_early() {
    let (_dir, layout, scope) = seeded();
    crate::cache::materialize_state(&layout).await.unwrap();
    let repo = Some(layout.root().to_path_buf());

    let unbounded = queries::trace(
        repo.clone(),
        &scope,
        serde_json::from_value::<TraceQuery>(serde_json::json!({
            "id": "source_schads", "direction": "both", "max_depth": 5, "limit": 200
        }))
        .unwrap(),
    )
    .await
    .unwrap();
    let bounded = queries::trace(
        repo,
        &scope,
        serde_json::from_value::<TraceQuery>(serde_json::json!({
            "id": "source_schads", "direction": "both", "max_depth": 5,
            "limit": 200, "visit_budget": 1
        }))
        .unwrap(),
    )
    .await
    .unwrap();

    assert!(
        bounded.nodes.len() < unbounded.nodes.len(),
        "a one-step budget must stop the walk before the unbounded answer: {} vs {}",
        bounded.nodes.len(),
        unbounded.nodes.len()
    );
}
