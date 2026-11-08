//! The one loader of the eleven record tables.
//!
//! Each table mirrors its record type, so the insert is built from
//! `ProjectionRow`: one bound value per column, in `COLUMNS` order. A kind
//! table takes one more column, `search_text`, derived from the record's
//! searchable pieces. Identifiers are quoted, since `key`, `field`,
//! `before`, and `after` are SQL keywords.

use super::batch::{text, Rows, Table};
use crate::cache::quoted;
use provenance_core::model::{ColumnValue, ProjectionRow};
use provenance_core::protocol::GraphNode;

/// The derived search column on a kind table.
pub const SEARCH_TEXT: &str = "search_text";

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

/// The quoted columns of one kind or integration table, with the search
/// column when the table has one.
fn columns<K: ProjectionRow>(with_search_text: bool) -> Vec<String> {
    let mut columns: Vec<String> = K::COLUMNS.iter().map(|column| quoted(column)).collect();
    if with_search_text {
        columns.push(quoted(SEARCH_TEXT));
    }
    columns
}

/// The values of one record, and its identity row when it is a graph node.
fn record_rows<K: Clone + ProjectionRow>(
    record: &K,
    wrap: Option<fn(Box<K>) -> GraphNode>,
) -> anyhow::Result<(Vec<ColumnValue>, Option<Vec<ColumnValue>>)> {
    let node = wrap.map(|wrap| wrap(Box::new(record.clone())));
    let mut values = record.row()?;
    let identity = node.map(|node| {
        values.push(text(search_text(&node)));
        vec![
            text(node.scope_id().as_str()),
            text(node.id().as_str()),
            text(node.node_type().as_str()),
        ]
    });
    Ok((values, identity))
}

/// The rows of one kind or integration family. `search_text` is given for
/// a kind table and absent for an integration table.
fn rows_of<K: Clone + ProjectionRow + serde::de::DeserializeOwned>(
    bytes: &[u8],
    wrap: Option<fn(Box<K>) -> GraphNode>,
) -> anyhow::Result<Rows> {
    let identity_columns = ["scope_id", "id", "node_type"].map(String::from);
    let mut rows = Rows::new(vec![
        Table::insert(&quoted(K::TABLE), &columns::<K>(wrap.is_some())),
        Table::insert("record_identities", &identity_columns),
    ]);
    for record in serde_json::from_slice::<Vec<K>>(bytes)? {
        match record_rows(&record, wrap) {
            Ok((values, identity)) => {
                rows.push(0, values);
                if let Some(identity) = identity {
                    rows.push(1, identity);
                }
                rows.loaded += 1;
            }
            Err(error) => {
                rows.failure = Some(error);
                break;
            }
        }
    }
    Ok(rows)
}

/// The rows of a kind family, with an identity row for each record.
pub(super) fn record_rows_of<K: Clone + ProjectionRow + serde::de::DeserializeOwned>(
    bytes: &[u8],
    wrap: fn(Box<K>) -> GraphNode,
) -> anyhow::Result<Rows> {
    rows_of(bytes, Some(wrap))
}

/// The rows of an integration family.
pub(super) fn kind_rows_of<K: Clone + ProjectionRow + serde::de::DeserializeOwned>(
    bytes: &[u8],
) -> anyhow::Result<Rows> {
    rows_of::<K>(bytes, None)
}
