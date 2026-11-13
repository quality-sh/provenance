//! Cases on the objects of the read database that are the store's.
//!
//! The store's objects are the objects that take a name that its schema
//! text names, and each index and trigger on a table of the store. The
//! schema check counts them, and the next catch-up removes one that the
//! schema text does not make. A table of another name, with its indexes
//! and its triggers, is not the store's: it stays with its rows.

use super::projection::{
    add_requirement, embedder_notes, foreign, outcome, repository, requirement_ids,
};
use super::{fetch, statement, write, Harness};
use crate::cache::catch_up_state;
use crate::read_database::{ReadDatabase, SqlRow, SqlValue};

/// Writes `sql` to `database` after a first catch-up, and then publishes
/// a second Requirement with the statement of the first. Gives the
/// objects and the rows of the table that the store does not own, as they
/// were before the catch-up.
async fn after_a_change(
    database: &dyn ReadDatabase,
    repository: &crate::repository::Repository,
    sql: &[&str],
) -> Vec<SqlRow> {
    embedder_notes(database).await;
    let kept = foreign(database).await;
    let first = catch_up_state(repository).await.unwrap();
    assert_eq!(outcome(&first), (1, true, true, true));
    write(
        database,
        sql.iter().map(|sql| statement(sql, Vec::new())).collect(),
    )
    .await
    .unwrap();
    add_requirement(repository, "req_second");
    kept
}

/// The next catch-up makes the projection again, with both Requirements,
/// and a catch-up after it finds the schema current.
async fn made_again(database: &dyn ReadDatabase, repository: &crate::repository::Repository) {
    let pass = catch_up_state(repository).await.unwrap();
    assert_eq!(
        outcome(&pass),
        (1, true, true, true),
        "the schema check did not ask for a rebuild"
    );
    assert_eq!(
        requirement_ids(database).await,
        vec![SqlValue::from("req_first"), SqlValue::from("req_second")]
    );
    let unchanged = catch_up_state(repository).await.unwrap();
    assert_eq!(outcome(&unchanged), (1, false, false, false));
}

/// A trigger of another name on a table of the store is the store's.
///
/// The trigger changes the id of each requirement that the store writes.
/// The next catch-up makes the table again without the trigger, so each
/// id reads back as the store wrote it. The table of another name beside
/// it keeps its rows, its index and its trigger.
pub async fn a_trigger_on_a_table_of_the_store_is_removed<H: Harness>(harness: H) {
    let repository = repository(&harness);
    let database = harness.database();
    let kept = after_a_change(
        database,
        &repository,
        &[
            "CREATE TRIGGER embedder_rewrite AFTER INSERT ON requirements \
           BEGIN UPDATE requirements SET id = id || '!' WHERE id = new.id; END",
        ],
    )
    .await;
    made_again(database, &repository).await;
    assert_eq!(
        fetch(
            database,
            "SELECT type, name FROM sqlite_schema WHERE name = 'embedder_rewrite'"
        )
        .await,
        Vec::<SqlRow>::new()
    );
    assert_eq!(foreign(database).await, kept);
}

/// An index of another name on a table of the store is the store's.
///
/// The unique index refuses two requirements with one statement. The next
/// catch-up makes the table again without the index, so the store writes
/// both. The table of another name beside it keeps its rows, its index and
/// its trigger.
pub async fn an_index_on_a_table_of_the_store_is_removed<H: Harness>(harness: H) {
    let repository = repository(&harness);
    let database = harness.database();
    let kept = after_a_change(
        database,
        &repository,
        &["CREATE UNIQUE INDEX embedder_one_statement ON requirements (scope_id, statement)"],
    )
    .await;
    made_again(database, &repository).await;
    assert_eq!(
        fetch(
            database,
            "SELECT type, name FROM sqlite_schema WHERE name = 'embedder_one_statement'"
        )
        .await,
        Vec::<SqlRow>::new()
    );
    assert_eq!(foreign(database).await, kept);
}

/// A view or a trigger that takes a name of the store is the store's.
///
/// A view named `topics` takes the place of that table, and a trigger on
/// the table of another name takes the name `requirements`. The next
/// catch-up drops both and makes the table `topics` again. The table of
/// another name keeps its rows, its index and its own trigger.
pub async fn an_object_that_takes_a_name_of_the_store_is_removed<H: Harness>(harness: H) {
    let repository = repository(&harness);
    let database = harness.database();
    let kept = after_a_change(
        database,
        &repository,
        &[
            "DROP TABLE topics",
            "CREATE VIEW topics AS SELECT 1 AS n",
            "CREATE TRIGGER requirements AFTER DELETE ON embedder_notes BEGIN SELECT 1; END",
        ],
    )
    .await;
    made_again(database, &repository).await;
    assert_eq!(
        fetch(
            database,
            "SELECT type, name FROM sqlite_schema \
             WHERE name IN ('topics', 'requirements') ORDER BY type, name"
        )
        .await,
        vec![
            vec![SqlValue::from("table"), SqlValue::from("requirements")],
            vec![SqlValue::from("table"), SqlValue::from("topics")],
        ]
    );
    assert_eq!(foreign(database).await, kept);
}
