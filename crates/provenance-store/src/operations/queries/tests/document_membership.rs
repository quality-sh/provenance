use super::{root_of, seeded_store};
use crate::cache::tests::fixtures::append_record;
use crate::operations::{queries, read_policy::ReadPolicy};
use provenance_core::{protocol::ReadDocumentQuery, ScopeId};
use provenance_macros::verifies;
use serde_json::{json, Value};

async fn page(root: &camino::Utf8Path, id: &str, cursor: Value) -> anyhow::Result<Value> {
    let result = queries::read_document(
        Some(root.to_owned()),
        &ScopeId::new("default")?,
        ReadPolicy::default(),
        serde_json::from_value::<ReadDocumentQuery>(json!({"id":id,"cursor":cursor,"limit":50}))?,
    )
    .await?;
    Ok(serde_json::to_value(
        provenance_core::protocol::QueryResponse::new("read-document", result),
    )?)
}
async fn entries(root: &camino::Utf8Path, id: &str) -> Vec<Value> {
    let mut cursor = Value::Null;
    let mut entries = vec![];
    let mut revision = None;
    loop {
        let page = page(root, id, cursor).await.unwrap();
        let current = (
            page["stamp"]["instance_id"].clone(),
            page["stamp"]["serial"].clone(),
            page["stamp"]["digest"].clone(),
        );
        assert_eq!(revision.get_or_insert_with(|| current.clone()), &current);
        entries.extend(page["entries"].as_array().unwrap().iter().cloned());
        cursor = page["next_cursor"].clone();
        if cursor.is_null() {
            return entries;
        }
    }
}

#[tokio::test]
#[verifies("rule_review_partial_reads_remain_explicit", examples)]
async fn discussion_pages_preserve_independent_threads_and_logical_order() {
    let (dir, store, scope) = seeded_store();
    for (id, status, counter) in [
        ("thread_later", "active", 8),
        ("thread_earlier", "resolved", 2),
    ] {
        append_record(
            &crate::shards::threads_path(&store.layout, &scope),
            &json!({
            "schema_version":2,"scope_id":"default","id":id,
            "parent":{"node_type":"requirement","node_id":"req_overtime"},"status":status,"created_at":counter}),
        );
    }
    let path = store
        .layout
        .state_dir()
        .join("scopes/default/threads/2026-09.jsonl");
    for i in 0..205 {
        append_record(
            &path,
            &json!({"schema_version":2,"scope_id":"default","id":format!("msg_{i:03}"),
            "thread_id":if i%2==0 {"thread_later"} else {"thread_earlier"},"role":"user", "body":"Saved discussion",
            "created_at":i/2,"ai_metadata":{"logical":true}}),
        );
    }
    let root = root_of(&dir);
    let first = page(&root, "req_overtime", Value::Null).await.unwrap();
    assert!(first["has_more"].as_bool().unwrap());
    let found = entries(&root, "req_overtime").await;
    let threads: Vec<_> = found.iter().filter(|e| e["kind"] == "thread").collect();
    assert_eq!(threads.len(), 2);
    assert_eq!(threads[0]["thread"]["id"], "thread_earlier");
    let messages: Vec<_> = found
        .iter()
        .filter(|e| e["kind"] == "message")
        .map(|e| &e["message"])
        .collect();
    assert_eq!(messages.len(), 205);
    assert!(messages
        .windows(2)
        .all(|p| (p[0]["created_at"].as_i64(), p[0]["id"].as_str())
            < (p[1]["created_at"].as_i64(), p[1]["id"].as_str())));
    append_record(
        &path,
        &json!({"schema_version":2,"scope_id":"default","id":"msg_new",
        "thread_id":"thread_earlier","role":"user","body":"New revision","created_at":200}),
    );
    assert!(page(&root, "req_overtime", first["next_cursor"].clone())
        .await
        .unwrap_err()
        .to_string()
        .contains("revision"));
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn member_work_budget_refuses_instead_of_claiming_a_complete_prefix() {
    let (dir, store, scope) = seeded_store();
    let path = crate::shards::requirements_path(&store.layout, &scope);
    let mut record = json!(store.list_requirements(&scope).unwrap()[0]);
    for i in 0..4100 {
        record["id"] = json!(format!("req_budget_{i:04}"));
        record["refines"] = json!("req_overtime");
        append_record(&path, &record);
    }
    let error = page(&root_of(&dir), "req_overtime", Value::Null)
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<provenance_core::protocol::read_failure::ReadFailure>(),
        Some(&provenance_core::protocol::read_failure::ReadFailure::PageBudgetExceeded)
    );
}

#[tokio::test]
async fn byte_limited_pages_continue_without_truncating_canonical_records() {
    let (dir, store, scope) = seeded_store();
    let path = crate::shards::requirements_path(&store.layout, &scope);
    let mut record = json!(store.list_requirements(&scope).unwrap()[0]);
    record["description"] = json!("x".repeat(40_000));
    for i in 0..35 {
        record["id"] = json!(format!("req_bytes_{i:03}"));
        record["refines"] = json!("req_overtime");
        append_record(&path, &record);
    }
    let root = root_of(&dir);
    let first = page(&root, "req_overtime", Value::Null).await.unwrap();
    assert!(serde_json::to_vec(&first).unwrap().len() <= super::super::page::RESPONSE_BYTES);
    assert!(first["entries"].as_array().unwrap().len() < 50);
    assert_eq!(first["has_more"], true);
    let all = entries(&root, "req_overtime").await;
    assert_eq!(
        all.iter()
            .filter(|e| e["kind"] == "member" && e["node"]["node_type"] == "requirement")
            .count(),
        36
    );
    for entry in all.iter().filter(|e| {
        e["node"]["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("req_bytes"))
    }) {
        assert_eq!(entry["node"]["description"].as_str().unwrap().len(), 40_000);
    }
}
