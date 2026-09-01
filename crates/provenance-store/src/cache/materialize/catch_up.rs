//! Truthful catch-up: hash everything, reparse only what changed.
//!
//! One pass, one guard scope, one transaction. The journal names what to
//! re-derive cheaply; the hash sweep over complete shard bytes is what makes
//! the freshness claim true, because a write journal cannot prove absence of
//! writes that bypass it. Size and mtime never license a skip.

use super::{family_rows, stamp};
use crate::cache::{open_cache, revision_digest_from_stored_rows, ProjectionFamily};
use crate::{
    canonical_digest, layout::ProvenanceLayout, migrations, publication, state_store::StateStore,
};
use provenance_core::ScopeId;
use std::collections::{BTreeMap, BTreeSet};

type UnitKey = (String, String);
type BaselineRow = (String, String, i64);
type StampRow = (String, String, String, i64);

/// What one catch-up pass did, in counters a caller can trust.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CatchUpReport {
    pub serial: i64,
    pub digest: String,
    pub rebuilt: bool,
    pub events_drained: u64,
    pub families_hashed: u64,
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

    // Step 1: a database with no revision, or one whose schema just moved,
    // cannot be caught up; it is rebuilt under the same held guard.
    let Some((stored_serial, _)) = stored else {
        return rebuild(guard, layout, &pool, migrations_applied).await;
    };
    if !migrations_applied.is_empty() {
        return rebuild(guard, layout, &pool, migrations_applied).await;
    }

    // Step 0 continued: repair the head, then freeze the drain window.
    let head = publication::normalize_head(layout, stored_serial)?;
    let events = publication::events_in_window(layout, stored_serial + 1, head - 1)?;
    let drained: BTreeSet<UnitKey> = events
        .iter()
        .map(|event| (event.family.clone(), event.scope.clone()))
        .collect();

    let snapshot = publication::snapshot_state_under_guard(guard, layout)?;
    let store = StateStore::new(snapshot.layout().clone());
    let manifest = store.manifest()?;
    // Rebuild's gatekeeper holds for catch-up too: an aggregate the
    // validator refuses commits nothing, whether or not its bytes moved.
    for scope in &manifest.scopes {
        store.validate_ideation_scope(&scope.id)?;
    }
    let mut scopes: Vec<ScopeId> = manifest
        .scopes
        .iter()
        .map(|scope| scope.id.clone())
        .collect();
    scopes.sort_by(|left, right| left.as_str().cmp(right.as_str()));

    let baseline: BTreeMap<UnitKey, BaselineRow> =
        sqlx::query_as::<_, (String, String, String, String, i64)>(
            "SELECT family, scope_id, digest, content_digest, record_count \
             FROM projection_family_digests",
        )
        .fetch_all(&pool)
        .await?
        .into_iter()
        .map(|(family, scope_id, digest, content, count)| {
            ((family, scope_id), (digest, content, count))
        })
        .collect();

    let mut report = CatchUpReport {
        serial: head,
        digest: String::new(),
        rebuilt: false,
        events_drained: events.len() as u64,
        families_hashed: 0,
        families_rederived: 0,
        rows_written: 0,
        migrations_applied,
    };
    let mut tx = pool.begin().await?;
    let sweep = Sweep {
        store: &store,
        snapshot_layout: snapshot.layout(),
        layout,
        scopes: &scopes,
        drained: &drained,
        baseline: &baseline,
    };
    let stamp_rows = sweep.run(&mut tx, &mut report).await?;
    remove_departed_units(&mut tx, &baseline, &stamp_rows).await?;

    report.digest = revision_digest_from_stored_rows(&stamp_rows)?;
    stamp::insert_revision(&mut tx, head, &report.digest).await?;
    publication::reserve_committed_serial(layout, head)?;
    crate::test_probes::at("catch_up_before_commit")?;
    tx.commit().await?;
    crate::test_probes::at("db_committed_before_head_fsync")?;
    publication::normalize_head(layout, head)?;
    publication::prune_up_to(layout, head)?;
    pool.close().await;
    Ok(report)
}

/// One sweep over every stored (family, scope): steps 2 and 3 share it.
///
/// Every unit's complete shard bytes are read and hashed; a drained unit, a
/// moved digest, or a missing baseline re-derives rows from those same
/// bytes. An unchanged unit keeps its rows and its stored content digest.
struct Sweep<'a> {
    store: &'a StateStore,
    snapshot_layout: &'a ProvenanceLayout,
    layout: &'a ProvenanceLayout,
    scopes: &'a [ScopeId],
    drained: &'a BTreeSet<UnitKey>,
    baseline: &'a BTreeMap<UnitKey, BaselineRow>,
}

impl Sweep<'_> {
    async fn run(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        report: &mut CatchUpReport,
    ) -> anyhow::Result<Vec<StampRow>> {
        let mut stamp_rows = Vec::new();
        for family in ProjectionFamily::ALL {
            let scope_keys: Vec<Option<&ScopeId>> = if family.is_scoped() {
                self.scopes.iter().map(Some).collect()
            } else {
                vec![None]
            };
            for scope in scope_keys {
                stamp_rows.push(self.sweep_unit(tx, report, family, scope).await?);
            }
        }
        Ok(stamp_rows)
    }

    async fn sweep_unit(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        report: &mut CatchUpReport,
        family: ProjectionFamily,
        scope: Option<&ScopeId>,
    ) -> anyhow::Result<StampRow> {
        let scope_name = scope.map_or("", ScopeId::as_str).to_string();
        let key = (family.family_name().to_string(), scope_name.clone());

        let domain = family.byte_domain(self.snapshot_layout, scope)?;
        let shard_digest = hash_unit(report, &crate::cache::domain_bytes(&domain)?)?;

        let stored_row = self.baseline.get(&key);
        let unchanged = stored_row
            .is_some_and(|(digest, content, _)| digest == &shard_digest && !content.is_empty())
            && !self.drained.contains(&key);

        let (content_digest, record_count) = if unchanged {
            let (_, content, count) = stored_row.expect("checked above");
            (content.clone(), *count)
        } else {
            family_rows::delete_rows(tx, family, scope).await?;
            let rows = family_rows::load_rows(tx, self.store, family, scope).await?;
            report.rows_written += rows;
            report.families_rederived += 1;
            let (content_bytes, count) = family.canonical_records(self.store, scope)?;
            (
                canonical_digest::digest(&content_bytes),
                i64::try_from(count)?,
            )
        };

        let (size_bytes, mtime_ns) = observed_metadata(self.layout, family, scope)?;
        stamp::insert_family_digest_row(
            tx,
            &scope_name,
            family.family_name(),
            &shard_digest,
            &content_digest,
            record_count,
            size_bytes,
            mtime_ns,
        )
        .await?;
        Ok((
            scope_name,
            family.family_name().to_string(),
            content_digest,
            record_count,
        ))
    }
}

/// The one way the sweep obtains a domain digest.
///
/// The counter and the observation probe live inside, so the report's
/// `families_hashed` is derived from hashes that actually ran; a sweep
/// that skips a family cannot claim its hash.
fn hash_unit(report: &mut CatchUpReport, bytes: &[u8]) -> anyhow::Result<String> {
    report.families_hashed += 1;
    crate::test_probes::at("catch_up_unit_hashed")?;
    Ok(canonical_digest::digest(bytes))
}

/// A (family, scope) with a baseline but no unit — a scope that left the
/// manifest — loses its rows and its digest row.
async fn remove_departed_units(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    baseline: &BTreeMap<UnitKey, BaselineRow>,
    stamp_rows: &[StampRow],
) -> anyhow::Result<()> {
    let live: BTreeSet<UnitKey> = stamp_rows
        .iter()
        .map(|(scope_id, family, ..)| (family.clone(), scope_id.clone()))
        .collect();
    for (family_name, scope_id) in baseline.keys() {
        if live.contains(&(family_name.clone(), scope_id.clone())) {
            continue;
        }
        let family = ProjectionFamily::ALL
            .into_iter()
            .find(|family| family.family_name() == family_name);
        if let Some(family) = family {
            let scope = if scope_id.is_empty() {
                None
            } else {
                Some(ScopeId::new(scope_id)?)
            };
            family_rows::delete_rows(tx, family, scope.as_ref()).await?;
        }
        sqlx::query("DELETE FROM projection_family_digests WHERE family = ? AND scope_id = ?")
            .bind(family_name)
            .bind(scope_id)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
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
        events_drained: 0,
        families_hashed: 0,
        families_rederived: 0,
        rows_written: report.records_loaded,
        migrations_applied,
    })
}

/// Size and mtime across the family's live byte domain: diagnostic only,
/// never a comparison input.
fn observed_metadata(
    layout: &ProvenanceLayout,
    family: ProjectionFamily,
    scope: Option<&ScopeId>,
) -> anyhow::Result<(i64, i64)> {
    crate::cache::domain_metadata(&family.byte_domain(layout, scope)?)
}
