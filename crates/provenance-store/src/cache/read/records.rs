//! The lookups of one typed table handle, and the kind of a bare id.

use super::rows::{decode, select_columns};
use super::BIND_CHUNK;
use crate::cache::materialize::SEARCH_TEXT;
use crate::cache::quoted;
use crate::operations::reader::{ReadSnapshot, Table};
use provenance_core::model::ProjectionRow;
use provenance_core::{
    Boundary, Domain, NodeType, Question, Requirement, Resolution, Rule, Source, StableId, Topic,
};
use provenance_macros::rule;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

/// The column that says a record retired, on the kinds that retire in
/// place.
const RETIRED: &str = "retired";

fn has_retired<K: ProjectionRow>() -> bool {
    K::COLUMNS.contains(&RETIRED)
}

/// The clause that leaves retired rows out, on a kind that has them. Every
/// lookup that decides whether a record counts goes through it, so a
/// retired record is answered only when the request asks for it.
#[rule("rule_retired_records_answer_only_when_asked")]
fn active_clause<K: ProjectionRow>(include_retired: bool) -> &'static str {
    if has_retired::<K>() && !include_retired {
        " AND retired = 0"
    } else {
        ""
    }
}

impl<K: ProjectionRow> Table<'_, K> {
    /// One record by id, retired or not.
    pub async fn record(&self, id: &StableId) -> anyhow::Result<Option<K>> {
        let sql = format!(
            "SELECT {} FROM {} WHERE scope_id = ? AND id = ?",
            select_columns::<K>(),
            quoted(K::TABLE)
        );
        let row = {
            let mut tx = self.snapshot().connection().await;
            sqlx::query(&sql)
                .bind(self.snapshot().scope().as_str())
                .bind(id.as_str())
                .fetch_optional(&mut **tx)
                .await?
        };
        row.as_ref().map(decode::<K>).transpose()
    }

    /// The records with the given ids that count under the view, one per
    /// id, in id order; a repeated id is answered once. The lookup is
    /// `by_field` on the id column, whose chunked select folds repeated
    /// values before it asks.
    #[rule("rule_by_ids_answers_a_repeated_id_once")]
    pub async fn by_ids(&self, ids: &[StableId], include_retired: bool) -> anyhow::Result<Vec<K>> {
        let wanted: Vec<&str> = ids.iter().map(StableId::as_str).collect();
        self.by_field("id", &wanted, include_retired).await
    }

    /// The records whose named column holds one of the values, under the
    /// view, in id order.
    pub async fn by_field(
        &self,
        column: &'static str,
        values: &[&str],
        include_retired: bool,
    ) -> anyhow::Result<Vec<K>> {
        let rows = self
            .rows_in(&select_columns::<K>(), column, values, include_retired)
            .await?;
        let mut records = rows
            .iter()
            .map(|row| Ok((row.try_get::<String, _>("id")?, decode::<K>(row)?)))
            .collect::<anyhow::Result<Vec<_>>>()?;
        records.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(records.into_iter().map(|(_, record)| record).collect())
    }

    /// The given ids that name a row that counts under the view, in id
    /// order.
    pub async fn ids_that_count(
        &self,
        ids: &[StableId],
        include_retired: bool,
    ) -> anyhow::Result<Vec<StableId>> {
        let wanted: Vec<&str> = ids.iter().map(StableId::as_str).collect();
        let rows = self.rows_in("id", "id", &wanted, include_retired).await?;
        let mut found = rows
            .iter()
            .map(|row| row.try_get::<String, _>(0))
            .collect::<Result<Vec<_>, _>>()?;
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
        include_retired: bool,
    ) -> anyhow::Result<Vec<SqliteRow>> {
        let mut values = values.to_vec();
        values.sort_unstable();
        values.dedup();
        let mut rows = Vec::new();
        for chunk in values.chunks(BIND_CHUNK) {
            let marks = vec!["?"; chunk.len()].join(", ");
            let sql = format!(
                "SELECT {select} FROM {} WHERE scope_id = ? AND {} IN ({marks}){}",
                quoted(K::TABLE),
                quoted(column),
                active_clause::<K>(include_retired)
            );
            let mut query = sqlx::query(&sql).bind(self.snapshot().scope().as_str());
            for value in chunk {
                query = query.bind(*value);
            }
            let fetched = {
                let mut tx = self.snapshot().connection().await;
                query.fetch_all(&mut **tx).await?
            };
            rows.extend(fetched);
        }
        Ok(rows)
    }

    /// Whether the id names a row that counts: present, and not retired
    /// unless retired rows are asked for.
    pub async fn live(&self, id: &StableId, include_retired: bool) -> anyhow::Result<bool> {
        let sql = format!(
            "SELECT 1 FROM {} WHERE scope_id = ? AND id = ?{}",
            quoted(K::TABLE),
            active_clause::<K>(include_retired)
        );
        let found: Option<i64> = {
            let mut tx = self.snapshot().connection().await;
            sqlx::query_scalar(&sql)
                .bind(self.snapshot().scope().as_str())
                .bind(id.as_str())
                .fetch_optional(&mut **tx)
                .await?
        };
        Ok(found.is_some())
    }

    /// The records whose search text holds the needle, in id order. The
    /// needle is folded to lowercase, as the search text is. The match is
    /// over the joined pieces; the caller decides per piece.
    pub async fn search(&self, needle: &str, include_retired: bool) -> anyhow::Result<Vec<K>> {
        let sql = format!(
            "SELECT {} FROM {} WHERE scope_id = ? AND instr({}, ?) > 0{} ORDER BY id",
            select_columns::<K>(),
            quoted(K::TABLE),
            quoted(SEARCH_TEXT),
            active_clause::<K>(include_retired)
        );
        let rows = {
            let mut tx = self.snapshot().connection().await;
            sqlx::query(&sql)
                .bind(self.snapshot().scope().as_str())
                .bind(needle.to_lowercase())
                .fetch_all(&mut **tx)
                .await?
        };
        rows.iter().map(decode::<K>).collect()
    }
}

/// The kind of a record named without one: the first kind, in rank
/// order, whose table holds a row that counts. Each probe takes its own
/// table handle, so the stamp names every table read.
pub async fn kind_of(
    snapshot: &ReadSnapshot,
    id: &StableId,
    include_retired: bool,
) -> anyhow::Result<Option<NodeType>> {
    macro_rules! probe {
        ($kind:ty, $node_type:expr) => {
            if snapshot.table::<$kind>().live(id, include_retired).await? {
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
    probe!(Domain, NodeType::Domain);
    probe!(Boundary, NodeType::Boundary);
    Ok(None)
}
