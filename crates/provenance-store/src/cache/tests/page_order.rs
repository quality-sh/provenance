//! Keyset ID pages walk the `(scope_id, id)` primary key. No page sorts
//! the rows after its key, and consecutive pages leave no gap.

use super::fixtures::{append_record, create_rule_of, seeded_layout};
use crate::cache::read::page::id_page_sql;
use crate::cache::read::payloads::PayloadRow;
use crate::cache::{catch_up_state, open_cache};
use crate::operations::reader::ReadSnapshot;
use crate::state_store::StateStore;
use provenance_core::model::ProjectionRow;
use provenance_core::{
    AssertionRecord, Boundary, Contribution, DispositionRecord, Domain, ImplementationBinding,
    Message, ProposalCard, Question, Requirement, RequirementReview, Resolution, Rule, Source,
    SynthesisPacket, Thread, Topic, VerificationBinding,
};
use serde_json::json;
use sqlx::{Row, SqlitePool};

async fn plan(pool: &SqlitePool, sql: &str) -> Vec<String> {
    sqlx::query(&format!("EXPLAIN QUERY PLAN {sql}"))
        .bind("default")
        .bind("")
        .bind(1_i64)
        .fetch_all(pool)
        .await
        .unwrap()
        .iter()
        .map(|row| row.try_get::<String, _>("detail").unwrap())
        .collect()
}

#[tokio::test]
async fn id_pages_search_the_primary_key_without_a_sort() {
    let (_dir, layout, _scope) = seeded_layout();
    catch_up_state(&layout).await.unwrap();
    let cache = open_cache(&layout).await.unwrap();
    for table in [
        Source::TABLE,
        Requirement::TABLE,
        Resolution::TABLE,
        Rule::TABLE,
        Topic::TABLE,
        Question::TABLE,
        Domain::TABLE,
        Boundary::TABLE,
        ImplementationBinding::TABLE,
        VerificationBinding::TABLE,
        RequirementReview::TABLE,
        <Thread as PayloadRow>::TABLE,
        <Message as PayloadRow>::TABLE,
        <Contribution as PayloadRow>::TABLE,
        <SynthesisPacket as PayloadRow>::TABLE,
        <ProposalCard as PayloadRow>::TABLE,
        <AssertionRecord as PayloadRow>::TABLE,
        <DispositionRecord as PayloadRow>::TABLE,
    ] {
        let details = plan(cache.pool(), &id_page_sql(table, "")).await;
        assert!(
            !details.iter().any(|detail| detail.contains("TEMP B-TREE")),
            "{table}: the page sorts its rows: {details:?}"
        );
        let index = format!("sqlite_autoindex_{table}_1 (scope_id=? AND id>?)");
        assert!(
            details.iter().any(|detail| detail.contains(&index)),
            "{table}: the page does not search the primary key: {details:?}"
        );
    }
    cache.close().await.unwrap();
}

#[tokio::test]
async fn id_pages_return_every_id_in_order() {
    let (_dir, layout, scope) = seeded_layout();
    let store = StateStore::new(layout.clone());
    for index in 0..10 {
        create_rule_of(
            &store,
            &scope,
            &format!("rule_page_{index:02}"),
            "req_schads_overtime",
        );
    }
    for index in 0..7 {
        append_record(
            &crate::shards::threads_path(&layout, &scope),
            &json!({
                "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION,
                "scope_id": scope.as_str(),
                "id": format!("thread_page_{index:02}"),
                "parent": {"node_type": "requirement", "node_id": "req_schads_overtime"},
                "status": "active",
                "created_at": index
            }),
        );
    }
    let mut rules: Vec<String> = store
        .list_rules(&scope)
        .unwrap()
        .iter()
        .map(|r| r.id.as_str().to_string())
        .collect();
    rules.sort();
    let mut threads: Vec<String> = store
        .list_threads(&scope)
        .unwrap()
        .iter()
        .map(|t| t.id.as_str().to_string())
        .collect();
    threads.sort();
    assert_eq!(threads.len(), 7);

    catch_up_state(&layout).await.unwrap();
    let cache = open_cache(&layout).await.unwrap();
    let snapshot = ReadSnapshot::open(cache.pool(), &scope)
        .await
        .unwrap()
        .expect("a revision");
    let mut walked: Vec<String> = Vec::new();
    loop {
        let after = walked.last().cloned().unwrap_or_default();
        let page = snapshot
            .table::<Rule>()
            .search_ids(&after, 3)
            .await
            .unwrap();
        assert!(page.len() <= 3);
        if page.is_empty() {
            break;
        }
        walked.extend(page);
    }
    assert_eq!(walked, rules);
    let mut walked: Vec<String> = Vec::new();
    loop {
        let after = walked.last().cloned().unwrap_or_default();
        let page = snapshot.payloads::<Thread>().ids(&after, 3).await.unwrap();
        assert!(page.len() <= 3);
        if page.is_empty() {
            break;
        }
        walked.extend(page);
    }
    assert_eq!(walked, threads);
    drop(snapshot);
    cache.close().await.unwrap();
}
