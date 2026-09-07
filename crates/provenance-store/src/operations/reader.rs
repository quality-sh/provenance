//! The one entry every query read goes through.
//!
//! A read takes the publication guard for the freshness step only. It then
//! answers from a snapshot pinned inside one `SQLite` read transaction, so it
//! never queues behind a publication or behind another reader. The stamp
//! on the answer is derived from the handles the read took: a projection
//! table is readable only through the snapshot, which records its family
//! word in `attested`, and a live part (canonical shards, the working-tree
//! scan, the run file, git) only through [`ReadContext::live`], which
//! records its word in `live`. The context is consumed to build the stamp,
//! so nothing reads after stamping.
//!
//! The context guards its transaction with an async mutex and its word
//! sets with plain mutexes, so the future a read runs is `Send`; the
//! operations read one statement at a time, so no lock is contended.

mod freshness;
mod live;
mod refuse_stale;
mod snapshot;

pub(crate) use freshness::is_missing_table;
pub use live::{Disturbed, Live, LiveHandle};
pub use refuse_stale::MovedUnit;
pub use snapshot::{ReadSnapshot, Relations, Table};

/// The projection readers that run over the handles: the fetched relation
/// front and the kind probe. The operations reach them from here, so a
/// query module never names the cache.
pub use crate::cache::read::{kind_of, SqlFront};

use super::read_policy::ReadPolicy;
use super::stamp;
use crate::layout::ProvenanceLayout;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::protocol::Stamped;
use provenance_core::ScopeId;
use provenance_macros::rule;
use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

/// The future one read runs over its context.
pub type ReadFuture<'c, R> = Pin<Box<dyn Future<Output = anyhow::Result<R>> + Send + 'c>>;

/// Why a read refused. A read with no stored revision refuses here and
/// names `provenance materialize`, under every freshness policy.
#[derive(Debug, thiserror::Error)]
#[rule("rule_no_revision_refuses_and_names_materialize")]
pub enum ReadRefusal {
    #[error("no projection revision in {database}; run `provenance materialize`{because}")]
    NoProjection {
        database: Utf8PathBuf,
        because: String,
    },
    #[error("refuse_stale: the projection in {} at serial {serial} (digest {digest}, instance {instance_id}) is behind canonical state; moved: {}. Read under catch_up or run `provenance materialize`.", one_line(.database.as_str()), refuse_stale::describe_moved(.moved))]
    Stale {
        database: Utf8PathBuf,
        serial: i64,
        digest: String,
        instance_id: String,
        moved: Vec<MovedUnit>,
    },
    #[error("refuse_stale: cannot hash {unit} at {}: {}", one_line(.path.as_str()), one_line(.error))]
    UnitUnreadable {
        unit: String,
        path: Utf8PathBuf,
        error: String,
    },
    #[error("the projection in {database} is behind on migrations or validation; run `provenance materialize`")]
    SchemaBehind { database: Utf8PathBuf },
    #[error("the projection in {database} holds a revision but no family digests, so its tables were never reloaded after a migration; run `provenance materialize`")]
    HalfMigrated { database: Utf8PathBuf },
}

fn one_line(text: &str) -> String {
    text.replace('\r', "\\r").replace('\n', "\\n")
}

/// Everything one read may reach: the pinned snapshot and the live parts.
pub struct ReadContext {
    snapshot: ReadSnapshot,
    live: Mutex<BTreeSet<Live>>,
    repo: Utf8PathBuf,
    scan_limit: usize,
}

impl ReadContext {
    fn new(snapshot: ReadSnapshot, repo: &Utf8Path, scan_limit: usize) -> Self {
        Self {
            snapshot,
            live: Mutex::new(BTreeSet::new()),
            repo: repo.to_path_buf(),
            scan_limit,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(snapshot: ReadSnapshot, repo: &Utf8Path) -> Self {
        Self::new(snapshot, repo, ReadPolicy::default().scan_limit)
    }

    pub const fn snapshot(&self) -> &ReadSnapshot {
        &self.snapshot
    }

    /// A handle on one live part; taking it puts the word on the stamp.
    #[rule("rule_stamp_lists_a_live_word_for_uncovered_parts")]
    pub fn live(&self, what: Live) -> LiveHandle<'_> {
        self.live
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(what);
        LiveHandle::new(self, what)
    }

    pub fn repo(&self) -> &Utf8Path {
        &self.repo
    }

    /// The file count the working-tree scan stops at.
    pub const fn scan_limit(&self) -> usize {
        self.scan_limit
    }

    pub(crate) fn into_parts(self) -> (ReadSnapshot, BTreeSet<Live>) {
        let live = self
            .live
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (self.snapshot, live)
    }
}

/// Runs one read: the freshness step under the guard, then `run` over a
/// pinned snapshot, then the stamp. Every query answer leaves through here,
/// so every answer carries a stamp.
#[rule("rule_query_answer_carries_a_stamp")]
pub async fn answer<R: Send>(
    repo: &Utf8Path,
    scope: &ScopeId,
    policy: ReadPolicy,
    run: impl for<'c> FnOnce(&'c ReadContext) -> ReadFuture<'c, R> + Send,
) -> anyhow::Result<Stamped<R>> {
    let layout = ProvenanceLayout::new(repo.to_path_buf());
    let fresh = freshness::run(&layout, scope, policy.freshness).await?;
    let opened = match fresh.snapshot {
        Some(snapshot) => Ok(Some(snapshot)),
        None => ReadSnapshot::open(&fresh.pool, scope).await,
    };
    let snapshot = match opened {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => {
            crate::cache::close_cache(&fresh.pool).await?;
            return Err(ReadRefusal::NoProjection {
                database: layout.cache_db_path(),
                because: fresh
                    .error
                    .as_deref()
                    .map(|error| format!(" (catch-up failed: {error})"))
                    .unwrap_or_default(),
            }
            .into());
        }
        Err(error) => {
            crate::cache::close_cache(&fresh.pool).await?;
            return Err(error);
        }
    };
    let context = ReadContext::new(snapshot, repo, policy.scan_limit);
    let result = run(&context).await;
    let stamp = stamp::seal(context, fresh.policy);
    crate::cache::close_cache(&fresh.pool).await?;
    Ok(Stamped {
        result: result?,
        stamp,
        freshness_error: fresh.error,
    })
}
