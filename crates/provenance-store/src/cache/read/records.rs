//! The lookups of one typed table handle, and the kind of a bare id.

use super::rows::{decode, id_column, select_columns};
use super::BIND_CHUNK;
use crate::cache::materialize::SEARCH_TEXT;
use crate::cache::quoted;
use crate::operations::reader::{ReadSnapshot, Table};
use crate::read_database::{SqlColumns, SqlRow, SqlValue};
use provenance_core::model::ProjectionRow;
use provenance_core::{
    ImplementationBinding, NodeType, Question, Requirement, Resolution, Rule, Source, StableId,
    Topic, VerificationBinding,
};
use provenance_macros::rule;

impl<K: ProjectionRow> Table<'_, K> {
    /// One record by id.
    pub async fn record(&self, id: &StableId) -> anyhow::Result<Option<K>> {
        let sql = format!(
            "SELECT {} FROM {} WHERE scope_id = ? AND id = ?",
            select_columns::<K>(),
            quoted(K::TABLE)
        );
        let rows = self
            .snapshot()
            .fetch(&sql, &[self.snapshot().scope_value(), id.as_str().into()])
            .await?;
        rows.first().map(|row| decode::<K>(row)).transpose()
    }

    /// The records with the given ids that count under the view, one per
    /// id, in id order; a repeated id is answered once. The lookup is
    /// `by_field` on the id column, whose chunked select folds repeated
    /// values before it asks.
    #[rule("rule_by_ids_answers_a_repeated_id_once")]
    pub async fn by_ids(&self, ids: &[StableId]) -> anyhow::Result<Vec<K>> {
        let wanted: Vec<&str> = ids.iter().map(StableId::as_str).collect();
        self.by_field("id", &wanted).await
    }

    /// The records whose named column holds one of the values, under the
    /// view, in id order.
    pub async fn by_field(&self, column: &'static str, values: &[&str]) -> anyhow::Result<Vec<K>> {
        let rows = self.rows_in(&select_columns::<K>(), column, values).await?;
        in_id_order(&rows)
    }

    /// Reads query-page records only after every stored row passes the
    /// record ceiling. Answers whose registered shape carries every
    /// match read here; one oversized row refuses the answer wherever it
    /// sits.
    pub(crate) async fn checked_by_field(
        &self,
        column: &'static str,
        values: &[&str],
    ) -> anyhow::Result<Vec<K>> {
        let mut values = values.to_vec();
        values.sort_unstable();
        values.dedup();
        let expression = super::page::byte_expression(K::COLUMNS);
        let maximum = i64::try_from(super::page::RECORD_BYTES)?;
        for chunk in values.chunks(BIND_CHUNK) {
            let marks = vec!["?"; chunk.len()].join(", ");
            let sql = format!(
                "SELECT {expression} AS record_bytes FROM {} WHERE scope_id = ? AND {} IN ({marks})",
                quoted(K::TABLE),
                quoted(column)
            );
            let rows = self.snapshot().fetch(&sql, &self.bound(chunk)).await?;
            let sizes = rows
                .iter()
                .map(|row| row.integer(0))
                .collect::<anyhow::Result<Vec<_>>>()?;
            if sizes.into_iter().any(|size| size > maximum) {
                return Err(
                    provenance_core::protocol::read_failure::ReadFailure::PageRecordTooLarge.into(),
                );
            }
        }
        self.by_field(column, &values).await
    }

    /// Every record with the given ids that counts under the view, each
    /// checked against the record ceiling before it decodes, in id order.
    /// The caller bounds the list, so the read stays within the page it
    /// serves.
    pub(crate) async fn checked_by_ids(&self, ids: &[StableId]) -> anyhow::Result<Vec<K>> {
        let wanted: Vec<&str> = ids.iter().map(StableId::as_str).collect();
        self.checked_by_field("id", &wanted).await
    }

    /// The first `limit` records whose named column holds one of the
    /// values, in id order, each checked against the record ceiling
    /// before it decodes, plus whether more matches remain. The page is
    /// chosen by id alone, so a match past the page never decodes and an
    /// oversized match past the page cannot refuse the answer. The
    /// condition, when given, is a SQL predicate over the table's own
    /// columns that a candidate must satisfy to count.
    pub(crate) async fn page_by_field(
        &self,
        column: &'static str,
        values: &[&str],
        condition: &'static str,
        limit: usize,
    ) -> anyhow::Result<(Vec<K>, bool)> {
        let mut values = values.to_vec();
        values.sort_unstable();
        values.dedup();
        let filter = match condition {
            "" => String::new(),
            condition => format!(" AND ({condition})"),
        };
        // Each value chunk answers with its smallest candidate ids; the
        // page is the smallest set over all chunks, so the global order
        // holds across the chunked statements.
        let mut candidates: Vec<String> = Vec::new();
        let wanted = i64::try_from(limit + 1)?;
        for chunk in values.chunks(BIND_CHUNK) {
            let marks = vec!["?"; chunk.len()].join(", ");
            let sql = format!(
                "SELECT id FROM {} WHERE scope_id = ? AND {} IN ({marks}){filter} ORDER BY id LIMIT ?",
                quoted(K::TABLE),
                quoted(column)
            );
            let mut values = self.bound(chunk);
            values.push(SqlValue::Integer(wanted));
            let rows = self.snapshot().fetch(&sql, &values).await?;
            for row in &rows {
                candidates.push(row.text(0)?.to_owned());
            }
        }
        candidates.sort();
        let more = candidates.len() > limit;
        candidates.truncate(limit);
        let records = self.page_rows(&candidates).await?;
        Ok((records, more))
    }

    /// The first `limit` records with the given ids that count under the
    /// view, in id order, plus whether more of the ids count.
    pub(crate) async fn page_by_ids(
        &self,
        ids: &[StableId],
        limit: usize,
    ) -> anyhow::Result<(Vec<K>, bool)> {
        let wanted: Vec<&str> = ids.iter().map(StableId::as_str).collect();
        self.page_by_field("id", &wanted, "", limit).await
    }

    /// The given ids that count, in id order, each after its stored byte
    /// count passes the cap. The caller bounds the list.
    async fn page_rows(&self, ids: &[String]) -> anyhow::Result<Vec<K>> {
        let wanted: Vec<&str> = ids.iter().map(String::as_str).collect();
        let expression = super::page::byte_expression(K::COLUMNS);
        let maximum = i64::try_from(super::page::RECORD_BYTES)?;
        for chunk in wanted.chunks(BIND_CHUNK) {
            let marks = vec!["?"; chunk.len()].join(", ");
            let sql = format!(
                "SELECT {expression} AS record_bytes FROM {} WHERE scope_id = ? AND id IN ({marks})",
                quoted(K::TABLE)
            );
            let rows = self.snapshot().fetch(&sql, &self.bound(chunk)).await?;
            let sizes = rows
                .iter()
                .map(|row| row.integer(0))
                .collect::<anyhow::Result<Vec<_>>>()?;
            if sizes.into_iter().any(|size| size > maximum) {
                return Err(
                    provenance_core::protocol::read_failure::ReadFailure::PageRecordTooLarge.into(),
                );
            }
        }
        let rows = self.rows_in(&select_columns::<K>(), "id", &wanted).await?;
        in_id_order(&rows)
    }

    /// The given ids that name a row that counts under the view, in id
    /// order.
    pub async fn ids_that_count(&self, ids: &[StableId]) -> anyhow::Result<Vec<StableId>> {
        let wanted: Vec<&str> = ids.iter().map(StableId::as_str).collect();
        let rows = self.rows_in("id", "id", &wanted).await?;
        let mut found = rows
            .iter()
            .map(|row| Ok(row.text(0)?.to_owned()))
            .collect::<anyhow::Result<Vec<_>>>()?;
        found.sort_unstable();
        found.into_iter().map(StableId::new).collect()
    }

    /// The rows whose named column holds one of the values, under the
    /// view, with `select` as the column list. The values go to the
    /// database in chunks, since one statement binds a bounded number of
    /// parameters; a repeated value is asked once, so no row comes back
    /// twice. The rows come back in the database's order.
    async fn rows_in(
        &self,
        select: &str,
        column: &str,
        values: &[&str],
    ) -> anyhow::Result<Vec<SqlRow>> {
        let mut values = values.to_vec();
        values.sort_unstable();
        values.dedup();
        let mut rows = Vec::new();
        for chunk in values.chunks(BIND_CHUNK) {
            let marks = vec!["?"; chunk.len()].join(", ");
            let sql = format!(
                "SELECT {select} FROM {} WHERE scope_id = ? AND {} IN ({marks})",
                quoted(K::TABLE),
                quoted(column)
            );
            rows.extend(self.snapshot().fetch(&sql, &self.bound(chunk)).await?);
        }
        Ok(rows)
    }

    /// Whether the id names a present row.
    pub async fn live(&self, id: &StableId) -> anyhow::Result<bool> {
        let sql = format!(
            "SELECT 1 FROM {} WHERE scope_id = ? AND id = ?",
            quoted(K::TABLE)
        );
        let rows = self
            .snapshot()
            .fetch(&sql, &[self.snapshot().scope_value(), id.as_str().into()])
            .await?;
        Ok(!rows.is_empty())
    }

    /// The records whose search text holds the needle, in id order. The
    /// needle is folded to lowercase, as the search text is. The match is
    /// over the joined pieces; the caller decides per piece.
    pub async fn search(&self, needle: &str) -> anyhow::Result<Vec<K>> {
        let sql = format!(
            "SELECT {} FROM {} WHERE scope_id = ? AND instr({}, ?) > 0 ORDER BY id",
            select_columns::<K>(),
            quoted(K::TABLE),
            quoted(SEARCH_TEXT)
        );
        let rows = self
            .snapshot()
            .fetch(
                &sql,
                &[self.snapshot().scope_value(), needle.to_lowercase().into()],
            )
            .await?;
        rows.iter().map(|row| decode::<K>(row)).collect()
    }

    /// The scope, then each value of one chunk, as a statement binds them.
    fn bound(&self, chunk: &[&str]) -> Vec<SqlValue> {
        std::iter::once(self.snapshot().scope_value())
            .chain(chunk.iter().map(|value| SqlValue::from(*value)))
            .collect()
    }
}

/// The records that the rows hold, in id order.
fn in_id_order<K: ProjectionRow>(rows: &[SqlRow]) -> anyhow::Result<Vec<K>> {
    let id = id_column::<K>()?;
    let mut records = rows
        .iter()
        .map(|row| Ok((row.text(id)?.to_owned(), decode::<K>(row)?)))
        .collect::<anyhow::Result<Vec<_>>>()?;
    records.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(records.into_iter().map(|(_, record)| record).collect())
}

pub struct ImplementationSymbolSite {
    pub rule_id: String,
    pub symbol: String,
}

pub struct VerificationSymbolSite {
    pub rule_id: String,
    pub symbol: Option<String>,
    pub method: String,
}

impl Table<'_, ImplementationBinding> {
    pub(crate) async fn implementation_sites_for_file(
        &self,
        file: &str,
    ) -> anyhow::Result<Vec<ImplementationSymbolSite>> {
        let rows = symbol_rows(self, "rule_id, symbol", file).await?;
        rows.iter()
            .map(|row| {
                Ok(ImplementationSymbolSite {
                    rule_id: row.text(0)?.to_owned(),
                    symbol: row.text(1)?.to_owned(),
                })
            })
            .collect()
    }
}

impl Table<'_, VerificationBinding> {
    pub(crate) async fn verification_sites_for_file(
        &self,
        file: &str,
    ) -> anyhow::Result<Vec<VerificationSymbolSite>> {
        let rows = symbol_rows(self, "rule_id, symbol, method", file).await?;
        rows.iter()
            .map(|row| {
                Ok(VerificationSymbolSite {
                    rule_id: row.text(0)?.to_owned(),
                    symbol: row.optional_text(1)?.map(str::to_owned),
                    method: row.text(2)?.to_owned(),
                })
            })
            .collect()
    }
}

/// The binding rows whose file column holds the file, with `select` as
/// the column list, in that order. Only the named columns are read, so no
/// binding record decodes. The callers pass a constant column list.
async fn symbol_rows<K: ProjectionRow>(
    table: &Table<'_, K>,
    select: &'static str,
    file: &str,
) -> anyhow::Result<Vec<SqlRow>> {
    let sql = format!(
        "SELECT {select} FROM {} WHERE scope_id = ? AND file = ? ORDER BY rule_id, id",
        quoted(K::TABLE)
    );
    let snapshot = table.snapshot();
    snapshot
        .fetch(&sql, &[snapshot.scope_value(), file.into()])
        .await
}

/// The kind of a record named without one: the first kind, in rank
/// order, whose table holds a row that counts. Each probe takes its own
/// table handle, so the stamp names every table read.
pub async fn kind_of(snapshot: &ReadSnapshot, id: &StableId) -> anyhow::Result<Option<NodeType>> {
    macro_rules! probe {
        ($kind:ty, $node_type:expr) => {
            if snapshot.table::<$kind>().live(id).await? {
                return Ok(Some($node_type));
            }
        };
    }
    probe!(Source, NodeType::Source);
    probe!(Requirement, NodeType::Requirement);
    probe!(Resolution, NodeType::Resolution);
    probe!(Rule, NodeType::Rule);
    probe!(Topic, NodeType::Topic);
    probe!(Question, NodeType::Question);
    Ok(None)
}
