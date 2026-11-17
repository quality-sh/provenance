use super::{append_separator_requirements, repository_at, root_of, seeded_store, SEPARATOR_TEXT};
use crate::operations::{queries, read_policy::ReadPolicy};
use provenance_core::protocol::{QueryResponse, SearchQuery};
use provenance_macros::verifies;
use serde_json::{json, Value};

async fn search(root: &camino::Utf8Path, request: Value) -> anyhow::Result<Value> {
    let result = queries::search(
        &repository_at(root),
        ReadPolicy::default(),
        serde_json::from_value::<SearchQuery>(request)?,
    )
    .await?;
    Ok(serde_json::to_value(QueryResponse::new("search", result))?)
}

#[tokio::test]
#[verifies("rule_cursor_binds_query_identity", examples)]
#[verifies("rule_cursor_restarts_on_revision_change", examples)]
async fn filter_only_cursor_pages_preserve_order_and_refuse_invalid_continuations() {
    let (dir, store, scope) = seeded_store();
    let root = root_of(&dir);
    let path = crate::shards::requirements_path(&store.layout, &scope);
    let mut record = json!(store.list_requirements(&scope).unwrap()[0]);
    for i in 0..205 {
        record["id"] = json!(format!("req_page_{i:03}"));
        crate::cache::tests::fixtures::append_record(&path, &record);
    }
    let request = json!({"node_types":["requirement"],"limit":200});
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
    let wrong_operation = queries::read_document(
        &repository_at(&root),
        ReadPolicy::default(),
        provenance_core::protocol::ReadDocumentQuery {
            id: "req_overtime".into(),
            exclude_terminal: false,
            cursor: Some(cursor.to_owned()),
            limit: 1,
        },
    )
    .await
    .unwrap_err();
    assert!(wrong_operation.to_string().contains("cursor"));
    let other_scope = provenance_core::ScopeId::new("other").unwrap();
    let manifest_path = store.layout.manifest_path();
    let mut manifest: Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest["scopes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"other","path_prefix":"."}));
    std::fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let wrong_scope = queries::search(
        &crate::repository::Repository::open(&root, other_scope.as_str()).unwrap(),
        ReadPolicy::default(),
        serde_json::from_value(json!({
            "node_types":["requirement"], "limit":200, "cursor":cursor
        }))
        .unwrap(),
    )
    .await
    .unwrap_err();
    assert!(wrong_scope.to_string().contains("cursor"));
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

/// The ids and the continuation flag of each page of a separator search.
async fn separator_pages(root: &camino::Utf8Path) -> Vec<(Vec<String>, bool)> {
    let request = json!({"text":SEPARATOR_TEXT,"node_types":["requirement"],"limit":50});
    let mut pages = vec![search(root, request.clone()).await.unwrap()];
    while let Some(cursor) = pages.last().unwrap()["next_cursor"].as_str() {
        let mut next = request.clone();
        next["cursor"] = json!(cursor);
        pages.push(search(root, next).await.unwrap());
    }
    pages
        .iter()
        .map(|page| {
            let ids = page["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|node| node["id"].as_str().unwrap().to_owned())
                .collect();
            (ids, page["has_more"].as_bool().unwrap())
        })
        .collect()
}

/// The separator requirement ids of the given numbers.
fn separator_ids(ids: impl Iterator<Item = usize>) -> Vec<String> {
    ids.map(|i| format!("req_{i:05}")).collect()
}

#[tokio::test]
#[verifies("rule_search_page_with_more_holds_a_record", examples)]
async fn a_separator_search_page_reads_past_failing_candidates_to_its_match() {
    let (dir, store, scope) = seeded_store();
    append_separator_requirements(&store, &scope, 700, |i| i == 699);
    assert_eq!(
        separator_pages(&root_of(&dir)).await,
        [(separator_ids(std::iter::once(699)), false)]
    );
}

#[tokio::test]
#[verifies("rule_search_page_with_more_holds_a_record", examples)]
async fn a_full_separator_page_with_more_matches_holds_its_records() {
    let (dir, store, scope) = seeded_store();
    // Each of the 60 matches comes before 600 candidates that fail the check
    // of each piece.
    append_separator_requirements(&store, &scope, 60 * 601, |i| i % 601 == 0);
    assert_eq!(
        separator_pages(&root_of(&dir)).await,
        [
            (separator_ids((0..50).map(|k| k * 601)), true),
            (separator_ids((50..60).map(|k| k * 601)), false),
        ]
    );
}

#[tokio::test]
#[verifies("rule_search_page_with_more_holds_a_record", examples)]
async fn a_full_separator_page_before_only_failing_candidates_has_no_continuation() {
    let (dir, store, scope) = seeded_store();
    append_separator_requirements(&store, &scope, 650, |i| i < 50);
    assert_eq!(
        separator_pages(&root_of(&dir)).await,
        [(separator_ids(0..50), false)]
    );
}

#[tokio::test]
#[verifies("rule_search_page_with_more_holds_a_record", examples)]
async fn a_page_stopped_by_the_byte_cap_holds_its_first_record() {
    let (dir, store, scope) = seeded_store();
    let root = root_of(&dir);
    let path = crate::shards::requirements_path(&store.layout, &scope);
    let mut record = json!(store.list_requirements(&scope).unwrap()[0]);
    // JSON writes each character of this text as six bytes, so two such
    // records fit in the page byte cap and three do not.
    record["description"] = json!("\u{2}".repeat(60_000));
    for i in 0..5 {
        record["id"] = json!(format!("req_large_{i}"));
        crate::cache::tests::fixtures::append_record(&path, &record);
    }
    let request = json!({"node_types":["requirement"]});
    let mut pages = vec![search(&root, request.clone()).await.unwrap()];
    while let Some(cursor) = pages.last().unwrap()["next_cursor"].as_str() {
        let mut next = request.clone();
        next["cursor"] = json!(cursor);
        pages.push(search(&root, next).await.unwrap());
    }
    let sizes: Vec<_> = pages
        .iter()
        .map(|page| page["nodes"].as_array().unwrap().len())
        .collect();
    assert_eq!(sizes, [2, 2, 2]);
    let ids: Vec<_> = pages
        .iter()
        .flat_map(|page| page["nodes"].as_array().unwrap())
        .map(|node| node["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "req_large_0",
            "req_large_1",
            "req_large_2",
            "req_large_3",
            "req_large_4",
            "req_overtime"
        ]
    );
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
        let pool = sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new().filename(store.layout.cache_db_path()),
        )
        .await
        .unwrap();
        sqlx::query(sql).execute(&pool).await.unwrap();
        pool.close().await;
        let result = queries::search(
            &repository_at(&root),
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
