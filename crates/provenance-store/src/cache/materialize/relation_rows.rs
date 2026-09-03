//! The derived relation table.
//!
//! Every row derives from one owner record's declared fields, or from the
//! links a topic or question carries, so the table has no digest row of its
//! own: it is rebuilt whenever an owner family of the scope is.

use crate::cache::serde_name;
use crate::state_store::StateStore;
use provenance_core::model::relations::{link_rows_of, rows_of, RelationRow};
use provenance_core::{NodeType, ScopeId};
use sqlx::{Sqlite, Transaction};

/// The rows of one scope, in the order the declarations list them.
fn scope_rows(store: &StateStore, scope: &ScopeId) -> anyhow::Result<Vec<RelationRow>> {
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
    rows.extend(rows_of(&store.list_boundaries(scope)?));
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

/// Writes the scope's rows. Two citations of one source with different
/// clauses are one row. The rows are derived, so they count toward no
/// family and no report.
pub(super) async fn load_rows(
    tx: &mut Transaction<'_, Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
) -> anyhow::Result<()> {
    for row in scope_rows(store, scope)? {
        sqlx::query(
            "INSERT OR IGNORE INTO relations (scope_id, owner_type, owner_id, relation, target_type, target_id) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(scope.as_str())
        .bind(serde_name(&row.owner_type)?)
        .bind(row.owner_id.as_str())
        .bind(&row.relation)
        .bind(serde_name(&row.target_type)?)
        .bind(row.target_id.as_str())
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}
