//! What each catch-up pass that changes rows gives over the file: the
//! report, and the row count and highest row id of every table of the
//! projection after it. The passes add records of each family, change
//! one, and add enough records of one family that its load takes more
//! than one statement. No Rule: the expected lines in
//! `catch_up_rows.txt`, captured on the code before the row writers moved
//! onto the read database port, hold the rows that each pass writes. On a
//! difference the test prints every line it read.

use super::comparison::test_stores::{self, TestStore};
use crate::cache::tests::fixtures;
use crate::cache::{catch_up_state, open_cache};
use provenance_core::RequirementStatus;
use sqlx::Row;

const EXPECTED: &str = include_str!("catch_up_rows.txt");

/// The report of one pass, without its digest, and one line per table.
async fn pass(store: &TestStore, label: &str, lines: &mut Vec<String>) {
    let line = match catch_up_state(&store.repository()).await {
        Ok(report) => format!(
            "{label}: serial {} rebuilt {} committed {} units {} families {} rows {} recreated {}",
            report.serial,
            report.rebuilt,
            report.revision_committed,
            report.units_hashed,
            report.families_rederived,
            report.rows_written,
            report.cache_recreated
        ),
        Err(error) => format!("{label}: refused: {error:#}"),
    };
    lines.push(line);
    let cache = open_cache(&store.layout()).await.unwrap();
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' \
         AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(cache.pool())
    .await
    .unwrap();
    for table in tables {
        let counted = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT count(*), coalesce(max(rowid), 0) FROM \"{table}\""
        )))
        .fetch_one(cache.pool())
        .await;
        let text = match counted {
            Ok(row) => format!(
                "count {} last rowid {}",
                row.get::<i64, _>(0),
                row.get::<i64, _>(1)
            ),
            Err(_) => {
                let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                    "SELECT count(*) FROM \"{table}\""
                )))
                .fetch_one(cache.pool())
                .await
                .unwrap();
                format!("count {count}")
            }
        };
        lines.push(format!("  {table}: {text}"));
    }
    cache.close().await.unwrap();
}

fn requirement(store: &TestStore, id: &str) {
    fixtures::create_requirement(
        &store.state_store(),
        &store.scope,
        id,
        RequirementStatus::Active,
    );
}

fn compare(lines: &[String]) {
    let actual = lines.join("\n") + "\n";
    if actual != EXPECTED {
        println!("=== CATCH-UP ROWS\n{actual}=== END CATCH-UP ROWS");
        for (index, (left, right)) in actual.lines().zip(EXPECTED.lines()).enumerate() {
            assert_eq!(left, right, "line {}", index + 1);
        }
        assert_eq!(actual.lines().count(), EXPECTED.lines().count());
    }
}

#[tokio::test]
async fn each_pass_that_changes_rows_writes_the_same_rows() {
    let store = test_stores::seeded_queries();
    let mut lines = Vec::new();
    pass(&store, "absent", &mut lines).await;
    requirement(&store, "req_rows_one");
    pass(&store, "one requirement", &mut lines).await;
    fixtures::create_rule_of(
        &store.state_store(),
        &store.scope,
        "rule_rows",
        "req_rows_one",
    );
    pass(&store, "one rule", &mut lines).await;
    fixtures::create_resolution(
        &store.state_store(),
        &store.scope,
        "res_rows",
        "req_rows_one",
    );
    pass(&store, "one resolution", &mut lines).await;
    fixtures::create_source(&store.state_store(), &store.scope, "src_rows");
    fixtures::attach_source(
        &store.state_store(),
        &store.scope,
        "req_rows_one",
        "src_rows",
    );
    pass(&store, "one source attached", &mut lines).await;
    fixtures::rewrite_records(
        &crate::shards::requirements_path(&store.layout(), &store.scope),
        |record| {
            if record["id"] == "req_rows_one" {
                record["statement"] = serde_json::json!("req_rows_one changed statement");
            }
        },
    );
    pass(&store, "one requirement changed", &mut lines).await;
    for index in 0..150 {
        requirement(&store, &format!("req_rows_many_{index:03}"));
    }
    pass(&store, "150 requirements", &mut lines).await;
    pass(&store, "unchanged", &mut lines).await;
    compare(&lines);
}
