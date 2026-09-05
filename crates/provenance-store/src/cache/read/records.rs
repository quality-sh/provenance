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
use sqlx::Row;

/// The column that says a record retired, on the kinds that retire in
/// place.
const RETIRED: &str = "retired";

fn has_retired<K: ProjectionRow>() -> bool {
    K::COLUMNS.contains(&RETIRED)
}

/// The clause that leaves retired rows out, on a kind that has them.
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

    /// The records with the given ids, one per id, in id order, retired or
    /// not. The ids go to the database in chunks, since one statement
    /// binds a bounded number of parameters; a repeated id is asked once.
    #[rule("rule_by_ids_answers_a_repeated_id_once")]
    pub async fn by_ids(&self, ids: &[StableId]) -> anyhow::Result<Vec<K>> {
        let mut wanted: Vec<&str> = ids.iter().map(StableId::as_str).collect();
        wanted.sort_unstable();
        wanted.dedup();
        let mut records: Vec<(String, K)> = Vec::new();
        for chunk in wanted.chunks(BIND_CHUNK) {
            let marks = vec!["?"; chunk.len()].join(", ");
            let sql = format!(
                "SELECT {} FROM {} WHERE scope_id = ? AND id IN ({marks})",
                select_columns::<K>(),
                quoted(K::TABLE)
            );
            let mut query = sqlx::query(&sql).bind(self.snapshot().scope().as_str());
            for id in chunk {
                query = query.bind(*id);
            }
            let rows = {
                let mut tx = self.snapshot().connection().await;
                query.fetch_all(&mut **tx).await?
            };
            for row in rows {
                records.push((row.try_get("id")?, decode::<K>(&row)?));
            }
        }
        records.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(records.into_iter().map(|(_, record)| record).collect())
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
