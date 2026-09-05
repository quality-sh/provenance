mod catch_up;
mod collaboration_records;
mod family_rows;
mod record_rows;
mod relation_rows;
mod stamp;
mod units;

pub use catch_up::catch_up_with_guard;
pub use catch_up::{catch_up_state, CatchUpReport};
pub use units::{unit_digest, units_for, Unit};

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
    crate::test_probes::at("run_migrations_under_guard")?;
    let migrations_applied = migrations::run_migrations(&pool, layout).await?;
    // Close rather than drop. A dropped pool releases its file handles
    // asynchronously, and on Windows a later delete of the database file
    // races that release.
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

/// The rebuild body for a caller that holds the guard.
///
/// Snapshot, validation, migrations, and the commit all run under the
/// guard. The serial is the stored serial plus one.
pub(super) async fn materialize_with_guard(
    guard: &publication::PublicationGuard,
    layout: &ProvenanceLayout,
) -> anyhow::Result<MaterializeReport> {
    let snapshot = publication::snapshot_state_under_guard(guard, layout)?;
    let store = StateStore::new(snapshot.layout().clone());
    let manifest = store.manifest()?;
    for scope in &manifest.scopes {
        store.validate_ideation_scope(&scope.id)?;
        store.validate_graph_scope(&scope.id)?;
    }
    let pool = open_cache(layout).await?;
    crate::test_probes::at("run_migrations_under_guard")?;
    let migrations_applied = migrations::run_migrations(&pool, layout).await?;
    let stored_serial: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(serial), 0) FROM projection_revision")
            .fetch_one(&pool)
            .await?;
    let mut tx = pool.begin().await?;
    clear_cache(&mut tx).await?;

    let mut records_loaded = 0;
    for scope in &manifest.scopes {
        for family in super::ProjectionFamily::ALL {
            records_loaded += family_rows::load_rows(&mut tx, &store, family, &scope.id).await?;
        }
        relation_rows::load_rows(&mut tx, &store, &scope.id).await?;
    }
    let scope_ids: Vec<_> = manifest
        .scopes
        .iter()
        .map(|scope| scope.id.clone())
        .collect();
    stamp::write_stamp(
        &mut tx,
        &store,
        snapshot.layout(),
        &scope_ids,
        stored_serial + 1,
    )
    .await?;
    crate::test_probes::at("materialize_before_commit")?;
    tx.commit().await?;
    crate::test_probes::at("materialize_after_commit")?;
    pool.close().await;

    Ok(MaterializeReport {
        records_loaded,
        migrations_applied,
    })
}

async fn clear_cache(tx: &mut Transaction<'_, Sqlite>) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM relations")
        .execute(&mut **tx)
        .await?;
    for family in super::ProjectionFamily::ALL {
        sqlx::query(&format!("DELETE FROM {}", family.family_name()))
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}
