//! Compare canonical bytes with one stored projection without updating it.

use super::freshness::{self, Check, Freshness};
use super::{ReadRefusal, ReadSnapshot};
use crate::cache::view_stored_digests;
#[cfg(feature = "host")]
use crate::cache::{scope_ids, unit_stored_digest, units_for};
#[cfg(feature = "host")]
use crate::publication::publication_guard;
use crate::read_database::{SqlColumns, Transaction};
use crate::repository::Repository;
use provenance_core::protocol::StampPolicy;
use provenance_macros::rule;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;

/// The freshness step of a `refuse_stale` read.
type FreshnessFuture<'r> = Pin<Box<dyn Future<Output = anyhow::Result<Freshness>> + Send + 'r>>;

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
/// Over a folder, the guard is taken before the read opens the database,
/// and the read holds it until the hashes are compared. A folder whose
/// guard this process cannot take is hashed twice with no lock, and the two
/// passes must agree. Any other storage gives one consistent view of its
/// state, so the read takes no guard and hashes the view once.
#[rule("rule_refuse_stale_writes_no_revision")]
pub(super) fn run(repository: &Repository) -> FreshnessFuture<'_> {
    Box::pin(async move {
        // A folder is local storage, which needs `host`.
        #[cfg(feature = "host")]
        let guard = if repository.has_local_storage() {
            match publication_guard(repository).await {
                Ok(guard) => Some(guard),
                Err(error) if crate::cache::permission_failure(repository.layout(), &error) => None,
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        #[cfg(not(feature = "host"))]
        let guard = None;
        let passes = if repository.has_local_storage() && guard.is_none() {
            2
        } else {
            1
        };
        Ok(Freshness {
            check: Check::Stale { guard, passes },
            policy: StampPolicy::RefuseStale,
            error: None,
        })
    })
}

/// A digest change refuses with the checked revision and each moved unit, in name order.
#[rule("rule_refuse_stale_names_the_moved_units")]
pub(super) async fn check(
    mut transaction: Transaction,
    repository: &Repository,
    passes: usize,
) -> anyhow::Result<ReadSnapshot> {
    let layout = repository.layout();
    let scope = repository.scope();
    freshness::ensure_current_schema(&mut transaction, layout).await?;
    let snapshot = ReadSnapshot::open(transaction, scope)
        .await?
        .ok_or_else(|| ReadRefusal::NoProjection {
            database: layout.cache_db_path(),
            because: String::new(),
        })?;
    let stored: BTreeMap<String, String> = snapshot
        .transaction()
        .await
        .fetch(
            "SELECT unit, stored_digest FROM projection_unit_digests",
            Vec::new(),
        )
        .await?
        .iter()
        .map(|row| Ok((row.text(0)?.to_owned(), row.text(1)?.to_owned())))
        .collect::<anyhow::Result<_>>()?;
    // A folder with no guard is hashed in two complete passes, which must match this
    // same stored revision. Each pass reads the scope list again. A missing lock or a
    // recovery marker does not prevent a consistent read-only image from answering.
    for _ in 0..passes {
        let moved = moved_units(repository, stored.clone())?;
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
    }
    Ok(snapshot)
}

fn moved_units(
    repository: &Repository,
    mut stored: BTreeMap<String, String>,
) -> anyhow::Result<Vec<MovedUnit>> {
    let mut moved = Vec::new();
    for (unit, live) in live_digests(repository)? {
        let stored = stored.remove(&unit).unwrap_or_default();
        if stored != live {
            moved.push(MovedUnit { unit, stored, live });
        }
    }
    moved.extend(stored.into_iter().map(|(unit, stored)| MovedUnit {
        unit,
        stored,
        live: String::new(),
    }));
    moved.sort_by(|left, right| left.unit.cmp(&right.unit));
    Ok(moved)
}

/// The stored digest of each unit of the canonical state, by unit name.
///
/// The state of a folder is read from its files with no lock. The state of
/// any other storage is read from one view of the storage.
fn live_digests(repository: &Repository) -> anyhow::Result<Vec<(String, String)>> {
    // A folder is local storage, which needs `host`.
    #[cfg(feature = "host")]
    if repository.has_local_storage() {
        return folder_digests(&repository.layout().state_dir());
    }
    view_stored_digests(&repository.storage().read_view()?)
}

/// The stored digest of each unit of the state files under `state_dir`.
#[cfg(feature = "host")]
fn folder_digests(state_dir: &camino::Utf8Path) -> anyhow::Result<Vec<(String, String)>> {
    units_for(&scope_ids(state_dir)?)
        .into_iter()
        .map(|unit| {
            let live = unit_stored_digest(state_dir, &unit).map_err(|error| {
                ReadRefusal::UnitUnreadable {
                    unit: unit.name(),
                    path: error.path,
                    error: format!("{:#}", error.error),
                }
            })?;
            Ok((unit.name(), live))
        })
        .collect()
}
