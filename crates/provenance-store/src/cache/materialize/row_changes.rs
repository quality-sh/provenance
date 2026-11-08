//! Writes only the rows of one family that differ from its stored rows.
//!
//! The rows of a family table in one scope are keyed by record id. A
//! record whose values equal its stored row keeps that row; a changed
//! record replaces its row; a new record adds a row and its identity row;
//! a record that is gone loses both. The table then holds the same rows as
//! after a delete and a full load. Reads order rows by their columns, never
//! by insertion, so the place of a written row does not change an answer.
//!
//! A load that a full write would refuse goes the full way, so the refusal
//! is the same: a loader that stopped on a record, two records with one id,
//! or a table with no id column.

use super::batch::{bind, Rows, Table};
use super::family_rows;
use crate::cache::ProjectionFamily;
use provenance_core::model::ColumnValue;
use provenance_core::ScopeId;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, Sqlite, Transaction, TypeInfo, ValueRef};
use std::collections::{BTreeMap, BTreeSet};

/// The most ids that one delete binds.
const IDS_PER_STATEMENT: usize = 500;

pub(super) async fn write_changes(
    tx: &mut Transaction<'_, Sqlite>,
    rows: Rows,
    family: ProjectionFamily,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    let key = rows.tables[0]
        .columns
        .iter()
        .position(|column| matches!(column.trim_matches('"'), "id" | "discussion_id"));
    let Some(key) = key.filter(|_| rows.failure.is_none()) else {
        return full(tx, rows, family, scope).await;
    };
    let mut fresh = BTreeMap::new();
    let mut identities = BTreeMap::new();
    for (table, values) in &rows.entries {
        let id = match values.get(if *table == 0 { key } else { 1 }) {
            Some(ColumnValue::Text(id)) => id.clone(),
            _ => return full(tx, rows, family, scope).await,
        };
        let target = if *table == 0 {
            &mut fresh
        } else {
            &mut identities
        };
        if target.insert(id, values.clone()).is_some() {
            return full(tx, rows, family, scope).await;
        }
    }
    let table = &rows.tables[0];
    let compared = compared_columns(table, key);
    let mut stored = stored_rows(tx, table, &compared, scope).await?;
    let mut write = Rows::new(rows.tables.iter().map(Table::again).collect());
    let mut replaced = Vec::new();
    for (table_index, values) in &rows.entries {
        if *table_index != 0 {
            continue;
        }
        let ColumnValue::Text(id) = &values[key] else {
            unreachable!("checked above")
        };
        match stored.remove(id) {
            Some(Some(before)) if compared.iter().map(|index| &values[*index]).eq(&before) => {}
            Some(_) => {
                replaced.push(id.clone());
                write.push(0, values.clone());
            }
            None => {
                write.push(0, values.clone());
                if let Some(identity) = identities.get(id) {
                    write.push(1, identity.clone());
                }
            }
        }
    }
    let removed: Vec<String> = stored.into_keys().collect();
    let gone: Vec<&String> = replaced.iter().chain(&removed).collect();
    delete(
        tx,
        &format!(
            "DELETE FROM {} WHERE scope_id = ? AND {} IN",
            table.name, table.columns[key]
        ),
        scope,
        None,
        &gone,
    )
    .await?;
    if let Some(node_type) = family.node_type() {
        let removed: Vec<&String> = removed.iter().collect();
        delete(
            tx,
            "DELETE FROM record_identities WHERE scope_id = ? AND node_type = ? AND id IN",
            scope,
            Some(node_type.as_str()),
            &removed,
        )
        .await?;
    }
    write.write(tx).await?;
    Ok(rows.loaded)
}

async fn full(
    tx: &mut Transaction<'_, Sqlite>,
    rows: Rows,
    family: ProjectionFamily,
    scope: &ScopeId,
) -> anyhow::Result<u64> {
    family_rows::delete_rows(tx, family, scope).await?;
    rows.write(tx).await
}

/// The columns whose values decide whether a stored row is current, the
/// key first. A collaboration table keeps the whole record as JSON in
/// `payload`, and its other columns come from that record, so the key and
/// the payload decide. Any other table compares every column.
fn compared_columns(table: &Table, key: usize) -> Vec<usize> {
    let payload = table.columns.iter().position(|column| column == "payload");
    let others = (0..table.columns.len()).filter(|index| *index != key);
    std::iter::once(key)
        .chain(others.filter(|index| payload.is_none_or(|payload| *index == payload)))
        .collect()
}

/// The stored values of the `compared` columns of one table in `scope`, by
/// id. A row with a value that this module cannot compare is `None`, so it
/// is written again.
async fn stored_rows(
    tx: &mut Transaction<'_, Sqlite>,
    table: &Table,
    compared: &[usize],
    scope: &ScopeId,
) -> anyhow::Result<BTreeMap<String, Option<Vec<ColumnValue>>>> {
    let columns: Vec<&str> = compared
        .iter()
        .map(|index| table.columns[*index].as_str())
        .collect();
    let sql = format!(
        "SELECT {} FROM {} WHERE scope_id = ?",
        columns.join(", "),
        table.name
    );
    let fetched = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(scope.as_str())
        .fetch_all(&mut **tx)
        .await?;
    let mut stored = BTreeMap::new();
    for row in &fetched {
        let id: String = row.try_get(0)?;
        stored.insert(id, values(row, compared.len()));
    }
    Ok(stored)
}

fn values(row: &SqliteRow, columns: usize) -> Option<Vec<ColumnValue>> {
    (0..columns)
        .map(|index| {
            let raw = row.try_get_raw(index).ok()?;
            if raw.is_null() {
                return Some(ColumnValue::Null);
            }
            match raw.type_info().name() {
                "INTEGER" => row.try_get(index).ok().map(ColumnValue::Integer),
                "REAL" => row.try_get(index).ok().map(ColumnValue::Real),
                "TEXT" => row.try_get(index).ok().map(ColumnValue::Text),
                _ => None,
            }
        })
        .collect()
}

/// Runs `prefix (?, ...)` over the ids in chunks, with the scope and, when
/// given, one more value bound before the ids.
async fn delete(
    tx: &mut Transaction<'_, Sqlite>,
    prefix: &str,
    scope: &ScopeId,
    second: Option<&str>,
    ids: &[&String],
) -> anyhow::Result<()> {
    let unique: BTreeSet<&String> = ids.iter().copied().collect();
    let ids: Vec<&String> = unique.into_iter().collect();
    for chunk in ids.chunks(IDS_PER_STATEMENT) {
        let marks = vec!["?"; chunk.len()].join(", ");
        let mut query =
            sqlx::query(sqlx::AssertSqlSafe(format!("{prefix} ({marks})"))).bind(scope.as_str());
        if let Some(second) = second {
            query = query.bind(second.to_owned());
        }
        for id in chunk {
            query = bind(query, &ColumnValue::Text((*id).clone()));
        }
        query.execute(&mut **tx).await?;
    }
    Ok(())
}
