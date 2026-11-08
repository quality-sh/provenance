//! The derived relation table.
//!
//! Every row derives from one owner record's declared fields, or from the
//! links a topic or question carries, so the table has no digest row of its
//! own: it is rebuilt whenever an owner family of the scope is.

use super::batch::{text, Rows, Table};
use crate::cache::{serde_name, ProjectionFamily};
use crate::state_store::GuardedStore;
use provenance_core::model::relations::{link_rows_of, rows_of, RelationRow};
use provenance_core::{NodeType, ScopeId};
use sqlx::{Sqlite, Transaction};
use std::collections::HashSet;

/// The families whose records declare relations.
pub(super) const OWNERS: [ProjectionFamily; 6] = [
    ProjectionFamily::Sources,
    ProjectionFamily::Requirements,
    ProjectionFamily::Resolutions,
    ProjectionFamily::Rules,
    ProjectionFamily::Topics,
    ProjectionFamily::Questions,
];

/// The rows of one scope, in the order the declarations list them.
pub(super) fn scope_rows(
    store: &GuardedStore<'_>,
    scope: &ScopeId,
) -> anyhow::Result<Vec<RelationRow>> {
    let mut rows = rows_of(&store.list_sources(scope)?);
    rows.extend(rows_of(&store.list_requirements(scope)?));
    rows.extend(rows_of(&store.list_resolutions(scope)?));
    rows.extend(rows_of(&store.list_rules(scope)?));
    let topics = store.list_topics(scope)?;
    rows.extend(rows_of(&topics));
    for topic in &topics {
        rows.extend(link_rows_of(NodeType::Topic, &topic.id, &topic.links));
    }
    let questions = store.list_questions(scope)?;
    rows.extend(rows_of(&questions));
    for question in &questions {
        rows.extend(link_rows_of(
            NodeType::Question,
            &question.id,
            &question.links,
        ));
    }
    Ok(rows)
}

pub(super) async fn delete_rows(
    tx: &mut Transaction<'_, Sqlite>,
    scope: &ScopeId,
) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM relations WHERE scope_id = ?")
        .bind(scope.as_str())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// One stored relation row of a scope: owner type, owner id, relation,
/// target type and target id.
pub(super) type StoredRow = (String, String, String, String, String);

/// The rows that [`load_rows`] stores for `rows`, in row order. Two
/// citations of one source with different clauses are one row, the first.
pub(super) fn stored_form(rows: &[RelationRow]) -> anyhow::Result<Vec<StoredRow>> {
    let mut seen = HashSet::new();
    let mut stored = Vec::with_capacity(rows.len());
    for row in rows {
        let key = (
            serde_name(&row.owner_type)?,
            row.owner_id.as_str().to_owned(),
            row.relation.clone(),
            serde_name(&row.target_type)?,
            row.target_id.as_str().to_owned(),
        );
        if seen.insert(key.clone()) {
            stored.push(key);
        }
    }
    Ok(stored)
}

/// Makes the stored rows of each owner kind in `owners` equal to the rows
/// of `rows` with that owner kind. Only the rows that differ are deleted or
/// written. The other kinds keep their rows, so a caller names every kind
/// whose records changed.
pub(super) async fn replace_rows(
    tx: &mut Transaction<'_, Sqlite>,
    rows: &[RelationRow],
    scope: &ScopeId,
    owners: &[String],
) -> anyhow::Result<()> {
    let fresh = stored_form(rows)?;
    let columns = [
        "scope_id",
        "owner_type",
        "owner_id",
        "relation",
        "target_type",
        "target_id",
    ]
    .map(String::from);
    let mut added = Rows::new(vec![Table::insert_or_ignore("relations", &columns)]);
    for owner in owners {
        let stored: Vec<StoredRow> = sqlx::query_as(
            "SELECT owner_type, owner_id, relation, target_type, target_id FROM relations \
             WHERE scope_id = ? AND owner_type = ?",
        )
        .bind(scope.as_str())
        .bind(owner)
        .fetch_all(&mut **tx)
        .await?;
        let wanted: HashSet<&StoredRow> = fresh.iter().filter(|row| &row.0 == owner).collect();
        let kept: HashSet<&StoredRow> = stored.iter().collect();
        for gone in stored.iter().filter(|row| !wanted.contains(row)) {
            sqlx::query(
                "DELETE FROM relations WHERE scope_id = ? AND owner_type = ? AND owner_id = ? \
                 AND relation = ? AND target_type = ? AND target_id = ?",
            )
            .bind(scope.as_str())
            .bind(&gone.0)
            .bind(&gone.1)
            .bind(&gone.2)
            .bind(&gone.3)
            .bind(&gone.4)
            .execute(&mut **tx)
            .await?;
        }
        for row in fresh
            .iter()
            .filter(|row| &row.0 == owner && !kept.contains(row))
        {
            added.push(
                0,
                vec![
                    text(scope.as_str()),
                    text(row.0.as_str()),
                    text(row.1.as_str()),
                    text(row.2.as_str()),
                    text(row.3.as_str()),
                    text(row.4.as_str()),
                ],
            );
        }
    }
    added.write(tx).await?;
    Ok(())
}

/// Writes the scope's rows. Two citations of one source with different
/// clauses are one row. The rows are derived, so they count toward no
/// family and no report.
pub(super) async fn load_rows(
    tx: &mut Transaction<'_, Sqlite>,
    rows: &[RelationRow],
    scope: &ScopeId,
) -> anyhow::Result<()> {
    let columns = [
        "scope_id",
        "owner_type",
        "owner_id",
        "relation",
        "target_type",
        "target_id",
    ]
    .map(String::from);
    let mut batch = Rows::new(vec![Table::insert_or_ignore("relations", &columns)]);
    for row in rows {
        batch.push(
            0,
            vec![
                text(scope.as_str()),
                text(serde_name(&row.owner_type)?),
                text(row.owner_id.as_str()),
                text(row.relation.as_str()),
                text(serde_name(&row.target_type)?),
                text(row.target_id.as_str()),
            ],
        );
    }
    batch.write(tx).await?;
    Ok(())
}
