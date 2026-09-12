//! Root-related key selection. Payload reads use the same bounded snapshot.
use crate::operations::reader::{Position, ReadSnapshot};
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_macros::rule;
use sqlx::Row;

// The recursive term follows only active refinement children. Production
// and references are separate sets, so a cross-link cannot widen the branch.
const SETS: &str = r"
WITH RECURSIVE
branch(id) AS (
 SELECT id FROM requirements WHERE scope_id = ?1 AND id = ?2
 UNION
 SELECT e.owner_id FROM branch b JOIN relations e
 ON e.scope_id = ?1 AND e.target_type = 'requirement' AND e.target_id = b.id
 AND e.owner_type = 'requirement' AND e.relation = 'refines'
 JOIN requirements r ON r.scope_id = ?1 AND r.id = e.owner_id
 LIMIT 4097
),
decisions(id) AS (
 SELECT DISTINCT e.owner_id FROM branch b JOIN relations e
 ON e.scope_id = ?1 AND e.target_type = 'requirement' AND e.target_id = b.id
 AND e.owner_type = 'resolution' AND e.relation = 'requirement_ids'
 JOIN resolutions r ON r.scope_id = ?1 AND r.id = e.owner_id LIMIT 4097
),
members(kind, id) AS (
 SELECT 'requirement', id FROM branch
 UNION SELECT 'resolution', id FROM decisions
 UNION SELECT 'rule', e.owner_id FROM branch b JOIN relations e
 ON e.scope_id = ?1 AND e.target_type = 'requirement' AND e.target_id = b.id
 AND e.owner_type = 'rule' AND e.relation = 'requirement_ids'
 JOIN rules r ON r.scope_id = ?1 AND r.id = e.owner_id
 UNION SELECT 'rule', e.owner_id FROM decisions d JOIN relations e
 ON e.scope_id = ?1 AND e.target_type = 'resolution' AND e.target_id = d.id
 AND e.owner_type = 'rule' AND e.relation = 'resolution_ids'
 JOIN rules r ON r.scope_id = ?1 AND r.id = e.owner_id
 UNION SELECT e.owner_type, e.owner_id FROM branch b JOIN relations e
 ON e.scope_id = ?1 AND e.target_type = 'requirement' AND e.target_id = b.id
 AND e.relation = 'requirement_id' AND e.owner_type IN ('topic', 'question', 'boundary')
 LIMIT 4097
),
ancestors(id) AS (
 SELECT refines FROM requirements WHERE scope_id = ?1 AND id = ?2 AND refines IS NOT NULL
 UNION SELECT r.refines FROM ancestors a JOIN requirements r
 ON r.scope_id = ?1 AND r.id = a.id WHERE r.refines IS NOT NULL
 LIMIT 4097
),
refs(kind, id) AS (
 SELECT e.target_type, e.target_id FROM members m JOIN relations e
 ON e.scope_id = ?1 AND e.owner_type = m.kind AND e.owner_id = m.id
 UNION SELECT 'requirement', id FROM ancestors
 UNION SELECT e.owner_type, e.owner_id FROM members m JOIN relations e
 ON e.scope_id = ?1 AND e.target_type = m.kind AND e.target_id = m.id
 WHERE (e.relation = 'supersedes'
 AND ((e.owner_type = 'requirement' AND EXISTS (SELECT 1 FROM requirements r WHERE r.scope_id = ?1 AND r.id = e.owner_id))
 OR (e.owner_type = 'source' AND EXISTS (SELECT 1 FROM sources s WHERE s.scope_id = ?1 AND s.id = e.owner_id))
 OR e.owner_type = 'resolution'))
 OR (m.kind = 'requirement' AND e.relation IN ('refines', 'requirement_ids'))
 LIMIT 4097
),
discussions(id) AS (
 SELECT t.id FROM members m JOIN threads t
 ON t.scope_id = ?1 AND t.parent_type = m.kind AND t.parent_id = m.id
),
keys(stage, kind, id, counter) AS (
 SELECT 0, kind, id, 0 FROM members
 UNION ALL SELECT 1, kind, id, 0 FROM refs EXCEPT SELECT 1, kind, id, 0 FROM members
 UNION ALL SELECT 2, 'thread', t.id, t.created_at FROM discussions d
 JOIN threads t ON t.scope_id = ?1 AND t.id = d.id
 UNION ALL SELECT 3, 'message', m.id, m.created_at FROM discussions d
 JOIN messages m ON m.scope_id = ?1 AND m.thread_id = d.id
),
ordered AS (
 SELECT stage, kind, id, counter, CASE WHEN stage = 0 AND kind = 'requirement' AND id = ?2 THEN 0 ELSE CASE kind WHEN 'source' THEN 0 WHEN 'requirement' THEN 1
 WHEN 'resolution' THEN 2 WHEN 'rule' THEN 3 WHEN 'topic' THEN 4 WHEN 'question' THEN 5
 WHEN 'domain' THEN 6 WHEN 'boundary' THEN 7 ELSE 0 END + 1 END AS rank FROM keys
)
";

impl ReadSnapshot {
    /// Selects members from the root branch and leaves ancestors in references.
    #[rule("rule_review_ancestors_are_not_descendants")]
    pub(crate) async fn document_keys(
        &self,
        root: &str,
        after: &Position,
        limit: usize,
    ) -> anyhow::Result<Vec<(String, Position)>> {
        for family in [
            "requirements",
            "resolutions",
            "rules",
            "topics",
            "questions",
            "boundaries",
            "sources",
            "relations",
            "threads",
            "messages",
        ] {
            self.attest(family);
        }
        let mut tx = self.connection().await;
        let overflow: i64 = sqlx::query_scalar(&format!(
            "{SETS} SELECT \
            (SELECT count(*) FROM members) > 4096 OR (SELECT count(*) FROM refs) > 4096 \
            OR (SELECT count(*) FROM branch) > 4096 OR (SELECT count(*) FROM ancestors) > 4096 \
            OR (SELECT count(*) FROM decisions) > 4096"
        ))
        .bind(self.scope().as_str())
        .bind(root)
        .fetch_one(&mut **tx)
        .await?;
        if overflow != 0 {
            return Err(ReadFailure::PageBudgetExceeded.into());
        }
        let rows = sqlx::query(&format!(
            "{SETS} SELECT stage, kind, CASE WHEN length(CAST(id AS BLOB)) <= 1024 THEN id END AS id, counter, rank FROM ordered \
            WHERE (stage, rank, counter, id) > (?3, ?4, ?5, ?6) \
            ORDER BY stage, rank, counter, id LIMIT ?7"
        ))
        .bind(self.scope().as_str())
        .bind(root)
        .bind(i64::from(after.stage))
        .bind(i64::from(after.rank))
        .bind(after.counter)
        .bind(&after.id)
        .bind(i64::try_from(limit)?)
        .fetch_all(&mut **tx)
        .await?;
        drop(tx);
        rows.iter()
            .map(|row| {
                Ok((
                    row.try_get("kind")?,
                    Position {
                        stage: row.try_get("stage")?,
                        rank: row.try_get("rank")?,
                        counter: row.try_get("counter")?,
                        id: row
                            .try_get::<Option<String>, _>("id")?
                            .ok_or(ReadFailure::PageRecordTooLarge)?,
                    },
                ))
            })
            .collect()
    }
}
