mod catch_up;
mod collaboration_records;
mod family_rows;
mod graph_records;
mod integration_records;
mod stamp;

pub use catch_up::{catch_up_state, CatchUpReport};

use super::{open_cache, MaterializeReport};
use crate::{layout::ProvenanceLayout, migrations, publication, state_store::StateStore};
use sqlx::{Sqlite, Transaction};

pub async fn materialize_empty_state(
    layout: &ProvenanceLayout,
) -> anyhow::Result<MaterializeReport> {
    let guard = publication::publication_guard(layout).await?;
    let _held = &guard;
    crate::test_probes::at("materialize_empty_under_guard")?;
    let pool = open_cache(layout).await?;
    let migrations_applied = migrations::run_migrations(&pool, layout).await?;
    // Close, never just drop: a dropped pool releases its file handles
    // asynchronously, and on Windows a later delete of the database races
    // that release.
    pool.close().await;
    Ok(MaterializeReport {
        records_loaded: 0,
        migrations_applied,
    })
}

pub async fn materialize_state(layout: &ProvenanceLayout) -> anyhow::Result<MaterializeReport> {
    let guard = publication::publication_guard(layout).await?;
    materialize_with_guard(&guard, layout).await
}

/// The rebuild body, for a holder that already owns the guard.
///
/// Snapshot, validation, migrations, the row transaction, and the commit
/// all run inside the caller's guard scope, so no canonical publication can
/// interleave with the projection write.
pub(super) async fn materialize_with_guard(
    guard: &publication::PublicationGuard,
    layout: &ProvenanceLayout,
) -> anyhow::Result<MaterializeReport> {
    let snapshot = publication::snapshot_state_under_guard(guard, layout)?;
    let store = StateStore::new(snapshot.layout().clone());
    let manifest = store.manifest()?;
    for scope in &manifest.scopes {
        store.validate_ideation_scope(&scope.id)?;
    }
    let pool = open_cache(layout).await?;
    let migrations_applied = migrations::run_migrations(&pool, layout).await?;
    let stored_serial: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(serial), 0) FROM projection_revision")
            .fetch_one(&pool)
            .await?;
    let serial = publication::normalize_head(layout, stored_serial)?;
    let mut tx = pool.begin().await?;
    clear_cache(&mut tx).await?;

    let mut records_loaded = 0;
    for scope in &manifest.scopes {
        records_loaded += graph_records::load_scope(&mut tx, &store, &scope.id).await?;
        records_loaded += collaboration_records::load_scope(&mut tx, &store, &scope.id).await?;
        records_loaded += integration_records::load_scope(&mut tx, &store, &scope.id).await?;
    }
    records_loaded += graph_records::load_edges(&mut tx, &store).await?;
    let scope_ids: Vec<_> = manifest
        .scopes
        .iter()
        .map(|scope| scope.id.clone())
        .collect();
    stamp::write_stamp(
        &mut tx,
        &store,
        snapshot.layout(),
        layout,
        &scope_ids,
        serial,
    )
    .await?;
    publication::reserve_committed_serial(layout, serial)?;
    crate::test_probes::at("materialize_before_commit")?;
    tx.commit().await?;
    crate::test_probes::at("db_committed_before_head_fsync")?;
    publication::normalize_head(layout, serial)?;
    publication::prune_up_to(layout, serial)?;
    pool.close().await;

    Ok(MaterializeReport {
        records_loaded,
        migrations_applied,
    })
}

async fn clear_cache(tx: &mut Transaction<'_, Sqlite>) -> anyhow::Result<()> {
    for family in super::ProjectionFamily::ALL {
        sqlx::query(&format!("DELETE FROM {}", family.family_name()))
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}
