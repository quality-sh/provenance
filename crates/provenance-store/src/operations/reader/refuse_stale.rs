//! Compare canonical bytes with one stored projection without updating it.

use super::freshness::{self, Freshness};
use super::{ReadRefusal, ReadSnapshot};
use crate::cache::{scope_ids, unit_digest, units_for};
use crate::layout::ProvenanceLayout;
use crate::publication::{publication_guard, publication_read_guard};
use provenance_core::protocol::StampPolicy;
use provenance_core::ScopeId;
use provenance_macros::rule;
use std::collections::BTreeMap;

/// A unit whose live bytes differ from the stored projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovedUnit {
    pub unit: String,
    pub stored: String,
    pub live: String,
}

pub(super) fn describe_moved(moved: &[MovedUnit]) -> String {
    moved
        .iter()
        .map(|unit| format!("{} (stored {}, live {})", unit.unit, unit.stored, unit.live))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The policy compares hashes without changing projection rows or revisions.
#[rule("rule_refuse_stale_writes_no_revision")]
pub(super) async fn run(layout: &ProvenanceLayout, scope: &ScopeId) -> anyhow::Result<Freshness> {
    let guard = match publication_guard(layout).await {
        Ok(guard) => guard,
        Err(error) if freshness::permission_failure(layout, &error) => {
            publication_read_guard(layout).await?
        }
        Err(error) => return Err(error),
    };
    let pool = freshness::open_stored(layout)
        .await
        .map_err(|error| freshness::no_projection(layout, &error))?;
    let checked = check(&pool, layout, scope).await;
    drop(guard);
    let checked = checked.and_then(|snapshot| {
        crate::test_probes::at("refuse_stale_after_hash")?;
        Ok(snapshot)
    });
    match checked {
        Ok(snapshot) => Ok(Freshness {
            pool,
            snapshot: Some(snapshot),
            policy: StampPolicy::RefuseStale,
            error: None,
        }),
        Err(error) => {
            pool.close().await;
            Err(error)
        }
    }
}

/// A digest change refuses with the checked revision and each moved unit, in name order.
#[rule("rule_refuse_stale_names_the_moved_units")]
async fn check(
    pool: &sqlx::SqlitePool,
    layout: &ProvenanceLayout,
    scope: &ScopeId,
) -> anyhow::Result<ReadSnapshot> {
    freshness::ensure_current_schema(pool, layout).await?;
    let snapshot =
        ReadSnapshot::open(pool, scope)
            .await?
            .ok_or_else(|| ReadRefusal::NoProjection {
                database: layout.cache_db_path(),
                because: String::new(),
            })?;
    let mut stored: BTreeMap<String, String> = {
        let mut tx = snapshot.connection().await;
        sqlx::query_as::<_, (String, String)>("SELECT unit, digest FROM projection_unit_digests")
            .fetch_all(&mut **tx)
            .await?
            .into_iter()
            .collect()
    };
    let state_dir = layout.state_dir();
    let scopes = scope_ids(&state_dir)?;
    let mut moved = Vec::new();
    for unit in units_for(&scopes) {
        let live = unit_digest(&state_dir, &unit).map_err(|error| ReadRefusal::UnitUnreadable {
            unit: unit.name(),
            path: error.path,
            error: format!("{:#}", error.error),
        })?;
        let stored = stored.remove(&unit.name()).unwrap_or_default();
        if stored != live {
            moved.push(MovedUnit {
                unit: unit.name(),
                stored,
                live,
            });
        }
    }
    moved.extend(stored.into_iter().map(|(unit, stored)| MovedUnit {
        unit,
        stored,
        live: String::new(),
    }));
    moved.sort_by(|left, right| left.unit.cmp(&right.unit));
    if !moved.is_empty() {
        return Err(ReadRefusal::Stale {
            database: layout.cache_db_path(),
            serial: snapshot.serial(),
            digest: snapshot.digest().to_owned(),
            instance_id: snapshot.instance_id().to_owned(),
            moved,
        }
        .into());
    }
    Ok(snapshot)
}
