mod collaboration_records;
mod graph_records;
mod integration_records;
mod stamp;

use super::{open_cache, MaterializeReport};
use crate::{layout::ProvenanceLayout, migrations, publication, state_store::StateStore};
use sqlx::{Sqlite, Transaction};

pub async fn materialize_empty_state(
    layout: &ProvenanceLayout,
) -> anyhow::Result<MaterializeReport> {
    let pool = open_cache(layout).await?;
    let migrations_applied = migrations::run_migrations(&pool, layout).await?;
    Ok(MaterializeReport {
        records_loaded: 0,
        migrations_applied,
    })
}

pub async fn materialize_state(layout: &ProvenanceLayout) -> anyhow::Result<MaterializeReport> {
    let snapshot = publication::snapshot_state(layout)?;
    let store = StateStore::new(snapshot.layout().clone());
    let manifest = store.manifest()?;
    for scope in &manifest.scopes {
        store.validate_ideation_scope(&scope.id)?;
    }
    let pool = open_cache(layout).await?;
    let migrations_applied = migrations::run_migrations(&pool, layout).await?;
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
    stamp::write_stamp(&mut tx, &store, layout, &scope_ids).await?;
    tx.commit().await?;

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
