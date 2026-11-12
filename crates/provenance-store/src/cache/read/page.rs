//! Page limits apply before record decoding and transport serialization.
use super::rows::{decode, select_columns};
use crate::cache::quoted;
use crate::operations::reader::{ReadSnapshot, Table};
use crate::read_database::{OverBudget, SqlColumns, SqlRow, SqlValue};
use provenance_core::model::ProjectionRow;
use provenance_core::protocol::failure::{InvalidInputReason, OperationFailure};
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_macros::rule;

pub const RECORD_BYTES: usize = 65_536;
pub const PAGE_BYTES: usize = 1_048_576;
pub const RESOURCE_RECORD_BYTES: usize = PAGE_BYTES - 16_384;

/// The engine steps that the statements of one page may run, joins and
/// sorts included.
pub const PAGE_WORK_STEPS: u64 = 10_000_000;

impl ReadSnapshot {
    /// Bounds the work of the statements that follow in this page
    /// transaction to [`PAGE_WORK_STEPS`].
    #[rule("rule_query_pages_bound_shared_reads")]
    pub(crate) async fn bound_page_work(&self) -> anyhow::Result<()> {
        self.transaction().await.bound_work(PAGE_WORK_STEPS).await
    }
}

/// Refuses a page limit outside 1..=200 with the typed input refusal that
/// names the `limit` field.
pub fn check_limit(limit: usize) -> anyhow::Result<()> {
    if (1..=200).contains(&limit) {
        return Ok(());
    }
    Err(OperationFailure::InvalidInput {
        field: Some("limit".to_owned()),
        reason: InvalidInputReason::InvalidValue,
    }
    .into())
}

/// The page budget refusal for a statement that the store stopped for
/// the work bound; any other error as it is.
pub fn page_error(error: anyhow::Error) -> anyhow::Error {
    if error.downcast_ref::<OverBudget>().is_some() {
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

/// Returns the SQL text for one keyset page of at most `LIMIT` IDs after a
/// key, in primary-key order. An ID over 1024 bytes comes back as NULL.
///
/// Bind the scope, the previous key, values named by `filter`, and the limit,
/// in that order. `filter` is trusted SQL text from a fixed set of literals and
/// never holds caller input. `ORDER BY` names the table column: a bare `id`
/// binds to the guarded output alias, and `SQLite` then sorts every remaining
/// row for each page.
pub fn id_page_sql(table: &str, filter: &str) -> String {
    let table = quoted(table);
    format!(
        "SELECT CASE WHEN length(CAST(id AS BLOB)) <= 1024 THEN id END AS id \
         FROM {table} WHERE scope_id = ? AND id > ?{filter} ORDER BY {table}.id LIMIT ?"
    )
}

impl<K: ProjectionRow> Table<'_, K> {
    /// Reads one canonical row only after its stored byte count passes the cap.
    pub(crate) async fn page_record(&self, id: &str) -> anyhow::Result<Option<K>> {
        self.record_within(id, RECORD_BYTES).await
    }

    /// Reads one public resource within the resource-page byte ceiling.
    pub(crate) async fn resource_record(&self, id: &str) -> anyhow::Result<Option<K>> {
        self.record_within(id, RESOURCE_RECORD_BYTES).await
    }

    /// Reads one row by id only after its stored byte count is at most
    /// `bytes`.
    async fn record_within(&self, id: &str, bytes: usize) -> anyhow::Result<Option<K>> {
        let snapshot = self.snapshot();
        let values = vec![snapshot.scope_value(), id.into()];
        let sizes = snapshot
            .fetch(
                &format!(
                    "SELECT {} FROM {} WHERE scope_id = ? AND id = ?",
                    byte_expression(K::COLUMNS),
                    quoted(K::TABLE)
                ),
                values.clone(),
            )
            .await?;
        let maximum = i64::try_from(bytes)?;
        if let Some(size) = sizes.first() {
            if size.integer(0)? > maximum {
                return Err(ReadFailure::PageRecordTooLarge.into());
            }
        }
        let rows = snapshot
            .fetch(
                &format!(
                    "SELECT {} FROM {} WHERE scope_id = ? AND id = ?",
                    select_columns::<K>(),
                    quoted(K::TABLE)
                ),
                values,
            )
            .await?;
        rows.into_iter().next().map(decode::<K>).transpose()
    }

    /// Reads at most `limit` candidate IDs strictly after the previous key.
    pub(crate) async fn search_ids(
        &self,
        after: &str,
        limit: usize,
        exclude_terminal: bool,
    ) -> anyhow::Result<Vec<String>> {
        let filter = crate::families::projection_families::exclude_terminal_predicate(
            K::TABLE,
            exclude_terminal,
        );
        let sql = id_page_sql(K::TABLE, &filter);
        let values = vec![
            self.snapshot().scope_value(),
            after.into(),
            SqlValue::Integer(i64::try_from(limit)?),
        ];
        page_ids(self.snapshot().fetch(&sql, values).await?)
    }
}

impl Table<'_, provenance_core::VerificationBinding> {
    pub(crate) async fn resource_ids_for_rule(
        &self,
        after: &str,
        limit: usize,
        rule: Option<&provenance_core::StableId>,
    ) -> anyhow::Result<Vec<String>> {
        let filter = rule.map_or("", |_| " AND rule_id = ?");
        let sql = id_page_sql("verification_bindings", filter);
        let mut values = vec![self.snapshot().scope_value(), after.into()];
        if let Some(rule) = rule {
            values.push(rule.as_str().into());
        }
        values.push(SqlValue::Integer(i64::try_from(limit)?));
        page_ids(self.snapshot().fetch(&sql, values).await?)
    }
}

/// The ids of one keyset page. An id that the page gives as NULL is over
/// the id ceiling and refuses the page.
pub fn page_ids(rows: Vec<SqlRow>) -> anyhow::Result<Vec<String>> {
    rows.into_iter()
        .map(|mut row| {
            row.take_optional_text(0)?
                .ok_or_else(|| ReadFailure::PageRecordTooLarge.into())
        })
        .collect()
}
