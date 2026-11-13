//! The rows that the characterization writes straight into the read
//! database: rows that the pinned store has none of, and rows that make a
//! reader refuse.

use sqlx::SqlitePool;

const DISPOSITION: &str = r#"{"scope_id":"default","id":"ID","proposal_id":"PROPOSAL","decision":"accepted","actor":{"identity_type":"human","id":"reader"}}"#;

/// The bytes of the statement of `rule_wide`: over the record ceiling and
/// under the resource ceiling.
pub(super) const WIDE_STATEMENT_BYTES: usize = 80_000;

/// The id of a thread under `req_bottom` that is over the id ceiling.
pub(super) fn long_thread_id() -> String {
    format!("thread_z{}", "z".repeat(1100))
}

pub(super) async fn run(pool: &SqlitePool, sql: &str, values: &[&str]) {
    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    for value in values {
        query = query.bind(*value);
    }
    query.execute(pool).await.unwrap();
}

/// Rows the pinned store has none of, and rows that make a reader refuse.
pub(super) async fn seed(pool: &SqlitePool) {
    let thread =
        "INSERT INTO threads (scope_id, id, parent_type, parent_id, status, created_at, payload) \
                  VALUES ('default', ?, 'requirement', ?, ?, ?, '{}')";
    run(pool, thread, &["thread_b", "req_top", "active", "20"]).await;
    run(pool, thread, &["thread_a", "req_top", "resolved", "20"]).await;
    run(pool, thread, &["thread_c", "req_left", "archived", "10"]).await;
    let message =
        "INSERT INTO messages (scope_id, id, thread_id, role, body, created_at, payload) \
                   VALUES ('default', ?, ?, ?, ?, ?, '{}')";
    run(
        pool,
        message,
        &["message_b", "thread_a", "user", "first", "5"],
    )
    .await;
    run(
        pool,
        message,
        &["message_a", "thread_b", "assistant", "second", "5"],
    )
    .await;
    run(
        pool,
        message,
        &["message_c", "thread_a", "system", "third", "1"],
    )
    .await;
    let large = "x".repeat(70_000);
    run(
        pool,
        message,
        &["message_large", "thread_c", "user", &large, "30"],
    )
    .await;
    let disposition = "INSERT INTO dispositions (scope_id, id, proposal_id, decision, rationale, actor, payload) \
                       VALUES ('default', ?, ?, 'accepted', '', 'reader', ?)";
    for (id, proposal) in [
        ("disposition_b", "proposal_one"),
        ("disposition_a", "proposal_two"),
        ("disposition_c", "proposal_one"),
    ] {
        let payload = DISPOSITION
            .replace("\"ID\"", &format!("\"{id}\""))
            .replace("PROPOSAL", proposal);
        run(pool, disposition, &[id, proposal, &payload]).await;
    }
    let huge = format!("{{\"padding\":\"{}\"}}", "y".repeat(1_100_000));
    run(
        pool,
        disposition,
        &["disposition_huge", "proposal_one", &huge],
    )
    .await;
    let long_id = format!("disposition_z{}", "z".repeat(1100));
    run(pool, disposition, &[&long_id, "proposal_two", "{}"]).await;
    run(
        pool,
        thread,
        &[&long_thread_id(), "req_bottom", "active", "40"],
    )
    .await;
    wide_rule(pool).await;
    run(
        pool,
        "UPDATE sources SET name = CAST(name AS BLOB) WHERE id = 'source_award_2019'",
        &[],
    )
    .await;
}

/// A copy of `rule_penalty_001` as `rule_wide`, with a statement over the
/// record ceiling.
async fn wide_rule(pool: &SqlitePool) {
    let columns: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info('rules')")
        .fetch_all(pool)
        .await
        .unwrap();
    let select = columns
        .iter()
        .map(|column| match column.as_str() {
            "id" => "'rule_wide'".to_owned(),
            "statement" => "?".to_owned(),
            other => format!("\"{other}\""),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let statement = "w".repeat(WIDE_STATEMENT_BYTES);
    run(
        pool,
        &format!("INSERT INTO rules SELECT {select} FROM rules WHERE id = 'rule_penalty_001'"),
        &[&statement],
    )
    .await;
}
