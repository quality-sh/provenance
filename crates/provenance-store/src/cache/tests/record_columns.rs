//! The eleven record tables mirror their record types: one column per
//! field, plus the derived `search_text` column on the eight kind tables.
//! A struct field with no column fails here until its migration lands.

use super::catch_up_behavior::assert_catch_up_equals_rebuild;
use super::fixtures::pinned_store::{pinned_store_layout, TWIN_ID};
use super::fixtures::seeded_layout;
use crate::cache::{materialize_state, open_cache};
use provenance_core::model::ProjectionRow;
use provenance_core::{
    Boundary, Domain, ImplementationBinding, Question, Requirement, RequirementReview, Resolution,
    Rule, Source, Topic, VerificationBinding,
};
use std::collections::BTreeSet;

async fn table_columns(pool: &sqlx::SqlitePool, table: &str) -> BTreeSet<String> {
    sqlx::query_scalar("SELECT name FROM pragma_table_info(?)")
        .bind(table)
        .fetch_all(pool)
        .await
        .unwrap()
        .into_iter()
        .collect()
}

/// The table's column set is the record's field set; a kind table adds
/// `search_text`. A set comparison, so a later `ADD COLUMN` passes.
async fn assert_mirrors<K: ProjectionRow>(pool: &sqlx::SqlitePool, kind: bool) {
    let mut expected: BTreeSet<String> = K::COLUMNS.iter().map(|c| (*c).to_string()).collect();
    if kind {
        expected.insert("search_text".into());
    }
    assert_eq!(
        table_columns(pool, K::TABLE).await,
        expected,
        "{} does not mirror its record type",
        K::TABLE
    );
}

#[tokio::test]
async fn every_kind_table_mirrors_its_record_columns() {
    let (_dir, layout, _scope) = seeded_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    assert_mirrors::<Source>(&pool, true).await;
    assert_mirrors::<Requirement>(&pool, true).await;
    assert_mirrors::<Resolution>(&pool, true).await;
    assert_mirrors::<Rule>(&pool, true).await;
    assert_mirrors::<Topic>(&pool, true).await;
    assert_mirrors::<Question>(&pool, true).await;
    assert_mirrors::<Domain>(&pool, true).await;
    assert_mirrors::<Boundary>(&pool, true).await;
    assert_mirrors::<ImplementationBinding>(&pool, false).await;
    assert_mirrors::<VerificationBinding>(&pool, false).await;
    assert_mirrors::<RequirementReview>(&pool, false).await;
    pool.close().await;
}

/// The search column holds the record's searchable pieces, lowercased,
/// in the order `GraphNode::searchable_text` lists them, joined by
/// `\u{1}`: the id, then the statement, then the name and description.
#[tokio::test]
async fn materialize_writes_search_text_from_searchable_text() {
    let (_dir, layout, _scope) = pinned_store_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let text: String =
        sqlx::query_scalar("SELECT search_text FROM rules WHERE id = 'rule_overtime_001'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        text,
        "rule_overtime_001\u{1}overtime is paid after the threshold\u{1}overtime threshold\u{1}pay overtime after the threshold"
    );
    pool.close().await;
}

/// Catch-up over an edited shard rewrites the widened rows the way a
/// full rebuild does, column for column.
#[tokio::test]
async fn catch_up_equals_rebuild_over_the_widened_tables() {
    let (_dir, layout, scope) = pinned_store_layout();
    materialize_state(&layout).await.unwrap();
    super::fixtures::rewrite_records(&crate::shards::rules_path(&layout, &scope), |record| {
        if record["id"] == "rule_overtime_001" {
            record["name"] = serde_json::Value::String("Overtime cap".into());
        }
    });
    assert_catch_up_equals_rebuild(&layout).await;
}

/// `links` may name one id under two kinds; the relation table keeps a
/// row for each kind.
#[tokio::test]
async fn a_link_under_two_kinds_keeps_both_relation_rows() {
    let (_dir, layout, _scope) = pinned_store_layout();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let kinds: Vec<String> = sqlx::query_scalar(
        "SELECT target_type FROM relations WHERE owner_id = 'topic_rates' AND relation = 'links' \
         AND target_id = ? ORDER BY target_type",
    )
    .bind(TWIN_ID)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(kinds, ["requirement", "rule"]);
    pool.close().await;
}
