//! Implementation aid, no Rule: the loads of a rebuild and a catch-up go
//! through `Rows::write`, and these tests show that the multi-row path and
//! the one-by-one path after a failed attempt keep the loader's order, the
//! row ids, and the first failing row's error, and that a row of the wrong
//! length fails the write before any statement runs.

use super::{text, Rows, Table};
use crate::read_database::file::FileReadDatabase;
use crate::read_database::{ReadDatabase, SqlRow, SqlValue};
use camino::Utf8PathBuf;
use provenance_core::model::ColumnValue;

/// Two tables: `t` with a text key and a value, `u` with a text key.
fn tables() -> Vec<Table> {
    let columns = |names: &[&str]| {
        names
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>()
    };
    vec![
        Table::insert("t", &columns(&["id", "v"])),
        Table::insert("u", &columns(&["id"])),
    ]
}

/// The rows of `ids` for `t` (table 0) and `u` (table 1), in that order.
fn rows(ids: &[(usize, &str)]) -> Rows {
    let mut rows = Rows::new(tables());
    for (table, id) in ids {
        let row: Vec<ColumnValue> = if *table == 0 {
            vec![text(*id), text(format!("value of {id}"))]
        } else {
            vec![text(*id)]
        };
        rows.push(*table, row);
    }
    rows.loaded = 7;
    rows
}

/// Writes `rows` with at most `bound_values` values a statement into new
/// tables, and gives the outcome and the `(rowid, id)` rows of each table.
async fn load(rows: Rows, bound_values: usize) -> (anyhow::Result<u64>, Vec<SqlRow>, Vec<SqlRow>) {
    let root = tempfile::tempdir().unwrap();
    let path = Utf8PathBuf::from_path_buf(root.path().to_path_buf()).unwrap();
    let layout = crate::layout::ProvenanceLayout::new(path);
    std::fs::create_dir_all(layout.cache_dir()).unwrap();
    let database = FileReadDatabase::new(layout);
    let mut seen = None;
    let slot = &mut seen;
    database
        .write(Box::new(move |mut tx| {
            Box::pin(async move {
                tx.execute("CREATE TABLE t (id TEXT PRIMARY KEY, v TEXT)", Vec::new())
                    .await?;
                tx.execute("CREATE TABLE u (id TEXT PRIMARY KEY)", Vec::new())
                    .await?;
                let outcome = rows.write(&mut tx, bound_values).await;
                let t = tx
                    .fetch("SELECT rowid, id FROM t ORDER BY rowid", Vec::new())
                    .await?;
                let u = tx
                    .fetch("SELECT rowid, id FROM u ORDER BY rowid", Vec::new())
                    .await?;
                *slot = Some((outcome, t, u));
                tx.commit().await
            })
        }))
        .await
        .unwrap();
    seen.unwrap()
}

fn ids(rows: &[(i64, &str)]) -> Vec<SqlRow> {
    rows.iter()
        .map(|(rowid, id)| vec![SqlValue::Integer(*rowid), SqlValue::from(*id)])
        .collect()
}

#[tokio::test]
async fn a_load_in_several_statements_keeps_the_order_of_each_table() {
    // Two values a row in `t` and a bound of 4: two statements of two rows
    // and one of one row.
    let rows = rows(&[
        (0, "a"),
        (1, "x"),
        (0, "b"),
        (0, "c"),
        (1, "y"),
        (0, "d"),
        (0, "e"),
    ]);
    let (outcome, t, u) = load(rows, 4).await;

    assert_eq!(outcome.unwrap(), 7);
    assert_eq!(t, ids(&[(1, "a"), (2, "b"), (3, "c"), (4, "d"), (5, "e")]));
    assert_eq!(u, ids(&[(1, "x"), (2, "y")]));
}

#[tokio::test]
async fn a_failed_attempt_inserts_one_by_one_up_to_the_first_failing_row() {
    // The second `a` fails its key in the second statement of `t`, after
    // the first statement ran in the attempt.
    let rows = rows(&[
        (0, "a"),
        (1, "x"),
        (0, "b"),
        (0, "c"),
        (1, "y"),
        (0, "a"),
        (0, "e"),
    ]);
    let (outcome, t, u) = load(rows, 4).await;

    let error = format!("{:#}", outcome.unwrap_err());
    assert!(error.contains("UNIQUE constraint failed: t.id"), "{error}");
    assert_eq!(t, ids(&[(1, "a"), (2, "b"), (3, "c")]));
    assert_eq!(u, ids(&[(1, "x"), (2, "y")]));
}

#[tokio::test]
async fn a_loader_failure_is_reported_after_its_rows_are_written() {
    let mut rows = rows(&[(0, "a"), (1, "x")]);
    rows.failure = Some(anyhow::anyhow!("the loader stopped"));
    let (outcome, t, u) = load(rows, 4).await;

    assert_eq!(outcome.unwrap_err().to_string(), "the loader stopped");
    assert_eq!(t, ids(&[(1, "a")]));
    assert_eq!(u, ids(&[(1, "x")]));
}

#[tokio::test]
async fn a_row_of_the_wrong_length_fails_the_write_before_any_statement() {
    // The second row of `t` has one value too many. Every row of the load
    // stays out, and so does the row of `u` before it.
    let mut rows = Rows::new(tables());
    rows.push(0, vec![text("a"), text("value of a")]);
    rows.push(1, vec![text("x")]);
    rows.push(0, vec![text("b"), text("value of b"), text("extra")]);
    rows.push(0, vec![text("c"), text("value of c")]);
    let (outcome, t, u) = load(rows, 4).await;

    assert_eq!(
        outcome.unwrap_err().to_string(),
        "a row for t has 3 values, but the table has 2 columns"
    );
    assert_eq!(t, ids(&[]));
    assert_eq!(u, ids(&[]));
}
