//! What a schema rebuild of the file cache leaves behind.
//!
//! No Rule: these tests are characterizations of the rebuild. The first
//! holds that a rebuild leaves every table of the store as the first
//! build made it, so the order in which the rebuild drops the objects of
//! the store is not visible to a reader. The second holds what a foreign
//! key from a table of another name does to the rebuild.

use super::fixtures::open;
use super::fixtures::pinned_store::pinned_store_layout;
use super::fixtures::seeded_layout;
use crate::cache::{catch_up_state, materialize_state, open_cache};
use crate::layout::ProvenanceLayout;

/// The tables of the store that hold one row by design.
const ONE_ROW: [&str; 4] = [
    "_cache_metadata",
    "projection_instance",
    "projection_revision",
    "projection_validation",
];

/// Each object of the catalog, then each row of each table, in a fixed
/// order, and the number of rows of each table.
async fn everything(layout: &ProvenanceLayout) -> (Vec<String>, Vec<(String, usize)>) {
    let cache = open_cache(layout).await.unwrap();
    let pool = cache.pool();
    let mut dump: Vec<String> = sqlx::query_scalar(
        "SELECT type || '|' || name || '|' || tbl_name || '|' || COALESCE(sql, '') \
         FROM sqlite_schema ORDER BY type, name",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_schema WHERE type = 'table' \
         AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let mut counts = Vec::new();
    for table in tables {
        let columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
                .bind(&table)
                .fetch_all(pool)
                .await
                .unwrap();
        let quoted: Vec<String> = columns
            .iter()
            .map(|column| format!("quote(\"{column}\")"))
            .collect();
        let select = format!(
            "SELECT {} FROM \"{table}\" ORDER BY 1",
            quoted.join(" || '|' || ")
        );
        let rows: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(select))
            .fetch_all(pool)
            .await
            .unwrap();
        counts.push((table.clone(), rows.len()));
        dump.extend(rows.into_iter().map(|row| format!("{table}: {row}")));
    }
    cache.close().await.unwrap();
    (dump, counts)
}

async fn execute(layout: &ProvenanceLayout, statements: &[&str]) {
    let cache = open_cache(layout).await.unwrap();
    for sql in statements {
        sqlx::query(sqlx::AssertSqlSafe(*sql))
            .execute(cache.pool())
            .await
            .unwrap();
    }
    cache.close().await.unwrap();
}

/// A schema rebuild and a full rebuild of a cache with more than one row
/// in each table of the store give the catalog and every row of every
/// table that the first build gave.
#[tokio::test]
async fn a_rebuild_leaves_every_table_as_the_first_build_made_it() {
    let (_dir, layout, _scope) = pinned_store_layout();
    let repository = open(&layout);
    let first = catch_up_state(&repository).await.unwrap();
    assert!(first.rebuilt);
    let (made, counts) = everything(&layout).await;
    let thin: Vec<&(String, usize)> = counts
        .iter()
        .filter(|(table, rows)| *rows < 2 && !ONE_ROW.contains(&table.as_str()))
        .collect();
    assert_eq!(thin, Vec::<&(String, usize)>::new(), "{counts:?}");
    execute(
        &layout,
        &["UPDATE _cache_metadata SET schema_digest = 'another'"],
    )
    .await;
    let pass = catch_up_state(&repository).await.unwrap();
    assert!(pass.rebuilt && pass.cache_recreated);
    assert_eq!(everything(&layout).await.0, made, "the schema rebuild");
    materialize_state(&repository).await.unwrap();
    assert_eq!(everything(&layout).await.0, made, "the full rebuild");
}

/// A table of another name with a foreign key into a table of the store
/// stops each rebuild while it holds a row that refers to a row of the
/// store: the file checks foreign keys, so the drop of the table of the
/// store fails. The table and its row stay. Once the row is removed, the
/// next catch-up rebuilds.
#[tokio::test]
async fn a_foreign_key_into_a_table_of_the_store_stops_each_rebuild() {
    let (_dir, layout, _scope) = seeded_layout();
    let repository = open(&layout);
    catch_up_state(&repository).await.unwrap();
    execute(
        &layout,
        &[
            "CREATE TABLE embedder_links (scope_id TEXT, id TEXT, \
             FOREIGN KEY (scope_id, id) REFERENCES requirements (scope_id, id))",
            "INSERT INTO embedder_links VALUES ('default', 'req_schads_overtime')",
            "UPDATE _cache_metadata SET schema_digest = 'another'",
        ],
    )
    .await;
    for pass in 0..2 {
        let failed = catch_up_state(&repository).await.unwrap_err();
        assert!(
            format!("{failed:#}").contains("FOREIGN KEY constraint failed"),
            "pass {pass}: {failed:#}"
        );
    }
    let links: Vec<String> = everything(&layout)
        .await
        .0
        .into_iter()
        .filter(|row| row.starts_with("embedder_links: "))
        .collect();
    assert_eq!(
        links,
        vec!["embedder_links: 'default'|'req_schads_overtime'"]
    );
    execute(&layout, &["DELETE FROM embedder_links"]).await;
    let pass = catch_up_state(&repository).await.unwrap();
    assert!(pass.rebuilt && pass.cache_recreated);
}
