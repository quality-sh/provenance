//! Catch-up: hash each scope unit and the global unit, then re-derive the
//! changed families by content digest.
//!
//! One pass runs under one guard in one transaction. A pass that changes
//! nothing commits no revision.

use super::units::{self, Unit};
use super::{family_rows, stamp};
use crate::cache::{open_cache, revision_digest_from_stored_rows, ProjectionFamily};
use crate::{
    canonical_digest, layout::ProvenanceLayout, migrations, publication, state_store::StateStore,
};
use provenance_core::ScopeId;
use std::collections::{BTreeMap, BTreeSet};

type ContentKey = (String, String);
type ContentValue = (String, i64);

/// What one catch-up pass did.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CatchUpReport {
    pub serial: i64,
    pub digest: String,
    pub rebuilt: bool,
    pub revision_committed: bool,
    pub units_hashed: u64,
    pub families_rederived: u64,
    pub rows_written: u64,
    pub migrations_applied: Vec<String>,
}

pub async fn catch_up_state(layout: &ProvenanceLayout) -> anyhow::Result<CatchUpReport> {
    let guard = publication::publication_guard(layout).await?;
    catch_up_with_guard(&guard, layout).await
}

async fn catch_up_with_guard(
    guard: &publication::PublicationGuard,
    layout: &ProvenanceLayout,
) -> anyhow::Result<CatchUpReport> {
    let pool = open_cache(layout).await?;
    crate::test_probes::at("run_migrations_under_guard")?;
    let migrations_applied = migrations::run_migrations(&pool, layout).await?;
    let stored: Option<(i64, String)> = sqlx::query_as(
        "SELECT serial, digest FROM projection_revision ORDER BY serial DESC LIMIT 1",
    )
    .fetch_optional(&pool)
    .await?;

    // A database with no revision, or one whose schema just moved, is
    // rebuilt under the same guard.
    let Some((stored_serial, stored_digest)) = stored else {
        return rebuild(guard, layout, &pool, migrations_applied).await;
    };
    if !migrations_applied.is_empty() {
        return rebuild(guard, layout, &pool, migrations_applied).await;
    }

    let snapshot = publication::snapshot_state_under_guard(guard, layout)?;
    let store = StateStore::new(snapshot.layout().clone());
    let manifest = store.manifest()?;
    // The same validation as a rebuild. A refusal commits nothing.
    for scope in &manifest.scopes {
        store.validate_ideation_scope(&scope.id)?;
        store.validate_graph_scope(&scope.id)?;
    }
    let scope_ids: Vec<ScopeId> = manifest
        .scopes
        .iter()
        .map(|scope| scope.id.clone())
        .collect();

    let (stored_units, mut content) = load_stored_digests(&pool).await?;
    let mut report = CatchUpReport {
        serial: stored_serial,
        digest: stored_digest,
        rebuilt: false,
        revision_committed: false,
        units_hashed: 0,
        families_rederived: 0,
        rows_written: 0,
        migrations_applied,
    };
    let mut tx = pool.begin().await?;

    let live = units::units_for(&scope_ids);
    let mut changed = remove_departed_scopes(&mut tx, &stored_units, &live, &mut content).await?;
    let state_dir = snapshot.layout().state_dir();
    for unit in &live {
        let digest = hash_unit(&mut report, &state_dir, unit)?;
        if stored_units.get(&unit.name()) == Some(&digest) {
            continue;
        }
        changed = true;
        apply_unit_change(&mut tx, &store, unit, &mut content, &mut report).await?;
        stamp::upsert_unit_row(&mut tx, &unit.name(), &digest).await?;
    }

    if !changed {
        drop(tx);
        pool.close().await;
        return Ok(report);
    }

    for ((scope_id, family), (digest, count)) in &content {
        stamp::upsert_content_row(&mut tx, scope_id, family, digest, *count).await?;
    }
    let rows: Vec<(String, String, String, i64)> = content
        .iter()
        .map(|((scope_id, family), (digest, count))| {
            (scope_id.clone(), family.clone(), digest.clone(), *count)
        })
        .collect();
    report.digest = revision_digest_from_stored_rows(&rows)?;
    report.serial = stored_serial + 1;
    stamp::insert_revision(&mut tx, report.serial, &report.digest).await?;
    report.revision_committed = true;
    crate::test_probes::at("catch_up_before_commit")?;
    tx.commit().await?;
    crate::test_probes::at("catch_up_after_commit")?;
    pool.close().await;
    Ok(report)
}

async fn load_stored_digests(
    pool: &sqlx::SqlitePool,
) -> anyhow::Result<(BTreeMap<String, String>, BTreeMap<ContentKey, ContentValue>)> {
    let units: BTreeMap<String, String> =
        sqlx::query_as::<_, (String, String)>("SELECT unit, digest FROM projection_unit_digests")
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect();
    let content: BTreeMap<ContentKey, ContentValue> =
        sqlx::query_as::<_, (String, String, String, i64)>(
            "SELECT scope_id, family, content_digest, record_count FROM projection_family_digests",
        )
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|(scope_id, family, digest, count)| ((scope_id, family), (digest, count)))
        .collect();
    Ok((units, content))
}

/// Removes the rows and digest rows of every scope the manifest does not
/// name. Edge rows belong to the global unit and stay. Returns whether
/// anything departed.
async fn remove_departed_scopes(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    stored_units: &BTreeMap<String, String>,
    live: &[Unit],
    content: &mut BTreeMap<ContentKey, ContentValue>,
) -> anyhow::Result<bool> {
    let live_names: BTreeSet<String> = live.iter().map(Unit::name).collect();
    let mut departed = false;
    for name in stored_units.keys() {
        if live_names.contains(name) {
            continue;
        }
        if let Some(scope) = Unit::scope_of(name)? {
            for family in ProjectionFamily::ALL.into_iter().filter(|f| f.is_scoped()) {
                family_rows::delete_rows(tx, family, Some(&scope)).await?;
            }
            content.retain(|(scope_id, _), _| scope_id != scope.as_str());
            sqlx::query("DELETE FROM projection_family_digests WHERE scope_id = ?")
                .bind(scope.as_str())
                .execute(&mut **tx)
                .await?;
        }
        sqlx::query("DELETE FROM projection_unit_digests WHERE unit = ?")
            .bind(name)
            .execute(&mut **tx)
            .await?;
        departed = true;
    }
    Ok(departed)
}

/// The global unit reloads the edges table whole. A scope unit re-derives
/// the scope's families by content digest.
async fn apply_unit_change(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    store: &StateStore,
    unit: &Unit,
    content: &mut BTreeMap<ContentKey, ContentValue>,
    report: &mut CatchUpReport,
) -> anyhow::Result<()> {
    match unit {
        Unit::Global => {
            family_rows::delete_rows(tx, ProjectionFamily::Edges, None).await?;
            report.rows_written +=
                family_rows::load_rows(tx, store, ProjectionFamily::Edges, None).await?;
            report.families_rederived += 1;
            let (bytes, count) = ProjectionFamily::Edges.canonical_records(store, None)?;
            let row = (canonical_digest::digest(&bytes), i64::try_from(count)?);
            content.insert((String::new(), "edges".to_string()), row);
        }
        Unit::Scope(scope) => rederive_scope(tx, store, scope, content, report).await?,
    }
    Ok(())
}

/// Parses every scoped family of the scope again and rewrites only the
/// families whose content digest moved.
async fn rederive_scope(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    store: &StateStore,
    scope: &ScopeId,
    content: &mut BTreeMap<ContentKey, ContentValue>,
    report: &mut CatchUpReport,
) -> anyhow::Result<()> {
    for family in ProjectionFamily::ALL.into_iter().filter(|f| f.is_scoped()) {
        let (bytes, count) = family.canonical_records(store, Some(scope))?;
        let fresh = (canonical_digest::digest(&bytes), i64::try_from(count)?);
        let key = (scope.as_str().to_string(), family.family_name().to_string());
        if content.get(&key) == Some(&fresh) {
            continue;
        }
        family_rows::delete_rows(tx, family, Some(scope)).await?;
        report.rows_written += family_rows::load_rows(tx, store, family, Some(scope)).await?;
        report.families_rederived += 1;
        content.insert(key, fresh);
    }
    Ok(())
}

/// Every unit hash goes through here, so `units_hashed` counts real hashes.
fn hash_unit(
    report: &mut CatchUpReport,
    state_dir: &camino::Utf8Path,
    unit: &Unit,
) -> anyhow::Result<String> {
    report.units_hashed += 1;
    crate::test_probes::at("catch_up_unit_hashed")?;
    units::unit_digest(state_dir, unit)
}

async fn rebuild(
    guard: &publication::PublicationGuard,
    layout: &ProvenanceLayout,
    pool: &sqlx::SqlitePool,
    migrations_applied: Vec<String>,
) -> anyhow::Result<CatchUpReport> {
    let report = super::materialize_with_guard(guard, layout).await?;
    let (serial, digest): (i64, String) = sqlx::query_as(
        "SELECT serial, digest FROM projection_revision ORDER BY serial DESC LIMIT 1",
    )
    .fetch_one(pool)
    .await?;
    pool.close().await;
    Ok(CatchUpReport {
        serial,
        digest,
        rebuilt: true,
        revision_committed: true,
        units_hashed: 0,
        families_rederived: 0,
        rows_written: report.records_loaded,
        migrations_applied,
    })
}
