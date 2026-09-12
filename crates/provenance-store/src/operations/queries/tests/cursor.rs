use super::{root_of, seeded_store};
use crate::operations::{queries, read_policy::ReadPolicy};
use provenance_core::protocol::{QueryResponse, SearchQuery};
use provenance_macros::verifies;
use serde_json::{json, Value};

async fn search(root: &camino::Utf8Path, request: Value) -> anyhow::Result<Value> {
    let result = queries::search(
        Some(root.to_owned()),
        &provenance_core::ScopeId::new("default")?,
        ReadPolicy::default(),
        serde_json::from_value::<SearchQuery>(request)?,
    )
    .await?;
    Ok(serde_json::to_value(QueryResponse::new("search", result))?)
}

#[tokio::test]
#[verifies("rule_cursor_binds_query_identity", examples)]
#[verifies("rule_cursor_restarts_on_revision_change", examples)]
async fn cursor_pages_preserve_order_and_refuse_invalid_continuations() {
    let (dir, store, scope) = seeded_store();
    let root = root_of(&dir);
    let path = crate::shards::requirements_path(&store.layout, &scope);
    let mut record = json!(store.list_requirements(&scope).unwrap()[0]);
    for i in 0..205 {
        record["id"] = json!(format!("req_page_{i:03}"));
        crate::cache::tests::fixtures::append_record(&path, &record);
    }
    let request = json!({"text":"req_","node_types":["requirement"],"limit":200});
    let first = search(&root, request.clone()).await.unwrap();
    let cursor = first["next_cursor"].as_str().expect("continuation");
    let mut next = request.clone();
    next["cursor"] = json!(cursor);
    let second = search(&root, next.clone()).await.unwrap();
    assert_eq!(second, search(&root, next.clone()).await.unwrap());
    let ids: Vec<_> = first["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .chain(second["nodes"].as_array().unwrap())
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 206);
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(second["next_cursor"].is_null());
    for (field, value) in [
        ("text", json!("overtime")),
        ("limit", json!(50)),
        ("node_types", json!(["rule"])),
    ] {
        let mut changed = next.clone();
        changed[field] = value;
        assert!(search(&root, changed)
            .await
            .unwrap_err()
            .to_string()
            .contains("cursor"));
    }
    let mut tampered = next.clone();
    tampered["cursor"] = json!(format!("{cursor}x"));
    assert!(search(&root, tampered).await.is_err());
    record["id"] = json!("req_new");
    crate::cache::tests::fixtures::append_record(&path, &record);
    assert!(search(&root, next)
        .await
        .unwrap_err()
        .to_string()
        .contains("revision"));
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn search_refuses_a_record_larger_than_the_engine_budget() {
    let (dir, store, scope) = seeded_store();
    let path = crate::shards::requirements_path(&store.layout, &scope);
    let mut record = json!(store.list_requirements(&scope).unwrap()[0]);
    record["description"] = json!("x".repeat(70_000));
    std::fs::write(path, format!("{record}\n")).unwrap();
    let result = search(&root_of(&dir), json!({"text":"req_overtime"})).await;
    assert!(
        result.is_err(),
        "oversize canonical records must be refused"
    );
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn a_bounded_empty_search_page_can_continue_to_an_unloaded_match() {
    let (dir, store, scope) = seeded_store();
    let path = crate::shards::requirements_path(&store.layout, &scope);
    let mut record = json!(store.list_requirements(&scope).unwrap()[0]);
    for i in 0..700 {
        record["id"] = json!(format!("req_{i:03}"));
        record["description"] = if i == 699 {
            json!("the needle")
        } else {
            Value::Null
        };
        crate::cache::tests::fixtures::append_record(&path, &record);
    }
    let first = search(
        &root_of(&dir),
        json!({"text":"needle","node_types":["requirement"]}),
    )
    .await
    .unwrap();
    assert!(first["nodes"].as_array().unwrap().is_empty());
    assert_eq!(first["has_more"], true);
    let second = search(
        &root_of(&dir),
        json!({"text":"needle","node_types":["requirement"],"cursor":first["next_cursor"]}),
    )
    .await
    .unwrap();
    assert_eq!(second["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(second["nodes"][0]["id"], "req_699");
    assert_eq!(second["has_more"], false);
}

#[tokio::test]
#[verifies("rule_cursor_restarts_on_revision_change", examples)]
async fn revision_comparison_checks_digest_and_instance_even_when_serial_matches() {
    use crate::operations::read_policy::FreshnessPolicy;
    use provenance_core::protocol::read_failure::ReadFailure;
    for sql in [
        "UPDATE projection_revision SET digest = 'changed'",
        "UPDATE projection_instance SET instance_id = 'changed'",
    ] {
        let (dir, store, scope) = seeded_store();
        let root = root_of(&dir);
        // Create an actual second match before establishing the tested cursor.
        let mut row = json!(store.list_requirements(&scope).unwrap()[0]);
        row["id"] = json!("req_second");
        crate::cache::tests::fixtures::append_record(
            &crate::shards::requirements_path(&store.layout, &scope),
            &row,
        );
        let first = search(&root, json!({"text":"overtime","limit":1}))
            .await
            .unwrap();
        assert!(first["next_cursor"].is_string());
        let pool = sqlx::SqlitePool::connect(&format!("sqlite://{}", store.layout.cache_db_path()))
            .await
            .unwrap();
        sqlx::query(sql).execute(&pool).await.unwrap();
        pool.close().await;
        let result = queries::search(
            Some(root),
            &scope,
            ReadPolicy {
                freshness: FreshnessPolicy::AnnotateOnly,
                ..ReadPolicy::default()
            },
            serde_json::from_value(
                json!({"text":"overtime","limit":1,"cursor":first["next_cursor"]}),
            )
            .unwrap(),
        )
        .await
        .unwrap_err();
        assert_eq!(
            result.downcast_ref::<ReadFailure>(),
            Some(&ReadFailure::CursorRevisionChanged)
        );
    }
}
