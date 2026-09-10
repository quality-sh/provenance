//! Page limits apply before record decoding and transport serialization.
use super::rows::{decode, select_columns};
use crate::cache::quoted;
use crate::operations::reader::{ReadSnapshot, Table};
use provenance_core::model::ProjectionRow;
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_macros::rule;
use sqlx::Row;

pub const RECORD_BYTES: usize = 65_536;
pub const PAGE_BYTES: usize = 1_048_576;

impl ReadSnapshot {
    /// Limits SQLite work in this page transaction, including joins and sorts.
    #[rule("rule_query_pages_bound_shared_reads")]
    pub(crate) async fn bound_page_work(&self) -> anyhow::Result<()> {
        let mut tx = self.connection().await;
        let mut handle = tx.lock_handle().await?;
        let mut calls = 0;
        handle.set_progress_handler(1000, move || {
            calls += 1;
            calls <= 10_000
        });
        drop(handle);
        drop(tx);
        Ok(())
    }
}

pub fn page_error(error: anyhow::Error) -> anyhow::Error {
    if error.downcast_ref::<sqlx::Error>().is_some_and(
        |error| matches!(error, sqlx::Error::Database(db) if db.code().as_deref() == Some("9")),
    ) {
        ReadFailure::PageBudgetExceeded.into()
    } else {
        error
    }
}

pub fn byte_expression(columns: &[&str]) -> String {
    columns
        .iter()
        .map(|c| format!("coalesce(length(CAST({} AS BLOB)), 0)", quoted(c)))
        .collect::<Vec<_>>()
        .join(" + ")
}

impl<K: ProjectionRow> Table<'_, K> {
    /// Reads one canonical row only after its stored byte count passes the cap.
    pub(crate) async fn page_record(&self, id: &str) -> anyhow::Result<Option<K>> {
        let mut tx = self.snapshot().connection().await;
        let size: Option<i64> = sqlx::query_scalar(&format!(
            "SELECT {} FROM {} WHERE scope_id = ? AND id = ?",
            byte_expression(K::COLUMNS),
            quoted(K::TABLE)
        ))
        .bind(self.snapshot().scope().as_str())
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?;
        let maximum = i64::try_from(RECORD_BYTES)?;
        if size.is_some_and(|size| size > maximum) {
            return Err(ReadFailure::PageRecordTooLarge.into());
        }
        let row = sqlx::query(&format!(
            "SELECT {} FROM {} WHERE scope_id = ? AND id = ?",
            select_columns::<K>(),
            quoted(K::TABLE)
        ))
        .bind(self.snapshot().scope().as_str())
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?;
        drop(tx);
        row.as_ref().map(decode::<K>).transpose()
    }

    /// Reads at most `limit` candidate IDs strictly after the previous key.
    pub(crate) async fn search_ids(
        &self,
        retired: bool,
        after: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<String>> {
        let active = if K::COLUMNS.contains(&"retired") && !retired {
            " AND retired = 0"
        } else {
            ""
        };
        let sql = format!(
            "SELECT CASE WHEN length(CAST(id AS BLOB)) <= 1024 THEN id END AS id FROM {} WHERE scope_id = ? AND id > ?{active} ORDER BY id LIMIT ?",
            quoted(K::TABLE)
        );
        let mut tx = self.snapshot().connection().await;
        let rows = sqlx::query(&sql)
            .bind(self.snapshot().scope().as_str())
            .bind(after)
            .bind(i64::try_from(limit)?)
            .fetch_all(&mut **tx)
            .await?;
        drop(tx);
        rows.iter()
            .map(|row| {
                row.try_get::<Option<String>, _>("id")?
                    .ok_or_else(|| ReadFailure::PageRecordTooLarge.into())
            })
            .collect()
    }
}
