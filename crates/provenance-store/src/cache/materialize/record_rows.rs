//! The one loader of the eleven record tables.
//!
//! Each table mirrors its record type, so the insert is built from
//! `ProjectionRow`: one bound value per column, in `COLUMNS` order. A kind
//! table takes one more column, `search_text`, derived from the record's
//! searchable pieces. Identifiers are quoted, since `key`, `field`,
//! `before`, and `after` are SQL keywords.

use provenance_core::model::{ColumnValue, ProjectionRow};
use provenance_core::protocol::GraphNode;
use sqlx::sqlite::SqliteArguments;
use sqlx::{query::Query, Sqlite, Transaction};

/// The derived search column on a kind table.
pub(super) const SEARCH_TEXT: &str = "search_text";

/// The separator between the pieces of `search_text`.
const SEARCH_TEXT_SEPARATOR: &str = "\u{1}";

/// The searchable pieces of a record, lowercased and joined, so a search
/// can prefilter rows with one `instr` call.
fn search_text(node: &GraphNode) -> String {
    node.searchable_text()
        .iter()
        .map(|piece| piece.to_lowercase())
        .collect::<Vec<_>>()
        .join(SEARCH_TEXT_SEPARATOR)
}

/// The search text of one kind's records, through the `GraphNode`
/// variant that carries the kind.
pub(super) fn kind_search<K: Clone>(wrap: fn(Box<K>) -> GraphNode) -> impl Fn(&K) -> String + Sync {
    move |record| search_text(&wrap(Box::new(record.clone())))
}

pub(super) fn quoted(identifier: &str) -> String {
    format!("\"{identifier}\"")
}

/// The insert for one record, with the search column when the table has
/// one.
fn insert_sql<K: ProjectionRow>(with_search_text: bool) -> String {
    let mut columns: Vec<String> = K::COLUMNS.iter().map(|column| quoted(column)).collect();
    if with_search_text {
        columns.push(quoted(SEARCH_TEXT));
    }
    let marks = vec!["?"; columns.len()].join(", ");
    format!(
        "INSERT INTO {} ({}) VALUES ({})",
        quoted(K::TABLE),
        columns.join(", "),
        marks
    )
}

fn bind<'q>(
    query: Query<'q, Sqlite, SqliteArguments<'q>>,
    value: ColumnValue,
) -> Query<'q, Sqlite, SqliteArguments<'q>> {
    match value {
        ColumnValue::Null => query.bind(None::<String>),
        ColumnValue::Integer(integer) => query.bind(integer),
        ColumnValue::Real(real) => query.bind(real),
        ColumnValue::Text(text) => query.bind(text),
    }
}

/// Writes the records of one kind or integration family. `search_text`
/// is given for a kind table and absent for an integration table.
pub(super) async fn load_kind<K: ProjectionRow>(
    tx: &mut Transaction<'_, Sqlite>,
    records: Vec<K>,
    search_text: Option<&(dyn Fn(&K) -> String + Sync)>,
) -> anyhow::Result<u64> {
    let sql = insert_sql::<K>(search_text.is_some());
    let mut loaded = 0;
    for record in records {
        let mut query = sqlx::query(&sql);
        for value in record.row()? {
            query = bind(query, value);
        }
        if let Some(search_text) = search_text {
            query = query.bind(search_text(&record));
        }
        query.execute(&mut **tx).await?;
        loaded += 1;
    }
    Ok(loaded)
}
