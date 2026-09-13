//! The freshness step a read runs before it answers.
//!
//! `catch_up` takes the publication guard, opens the connection inside it,
//! runs one catch-up pass, and drops the guard. When that step fails and the
//! database holds a revision, the read goes on at the stored serial with
//! the policy word `catch_up_failed` and the error text beside the answer.
//! `annotate_only` takes no guard and refuses an absent database.
//! `refuse_stale` hashes canonical units and refuses any digest change.
//! The connection each step returns is owned through completion, so no
//! step decides how a pool closes.

use super::{ReadContext, ReadFuture, ReadRefusal, ReadSnapshot};
use crate::cache::{catch_up_with_guard, open_cache, open_stored_cache, CacheConnection};
use crate::layout::ProvenanceLayout;
use crate::migrations;
use crate::operations::read_policy::FreshnessPolicy;
use crate::operations::stamp;
use crate::publication::publication_guard;
use camino::Utf8Path;
use provenance_core::protocol::{Stamp, StampPolicy, Stamped};
use provenance_core::ScopeId;
use provenance_macros::rule;

pub(super) struct Freshness {
    pub(super) connection: CacheConnection,
    pub(super) snapshot: Option<ReadSnapshot>,
    pub(super) policy: StampPolicy,
    pub(super) error: Option<String>,
}

impl Freshness {
    /// Answers from this freshness state and owns the connection through
    /// the read's completion. This is the one completion point of a read:
    /// the snapshot opens, the body runs, the stamp seals, and the awaited
    /// close reports before the answer does.
    pub(super) async fn complete<R: Send>(
        mut self,
        layout: &ProvenanceLayout,
        repo: &Utf8Path,
        scope: &ScopeId,
        scan_limit: usize,
        run: impl for<'c> FnOnce(&'c ReadContext) -> ReadFuture<'c, R> + Send,
    ) -> anyhow::Result<Stamped<R>> {
        let outcome = self
            .answer_at_snapshot(layout, repo, scope, scan_limit, run)
            .await;
        self.connection.close().await?;
        let (result, stamp) = outcome?;
        Ok(Stamped {
            result: result?,
            stamp,
            freshness_error: self.error,
        })
    }

    async fn answer_at_snapshot<R: Send>(
        &mut self,
        layout: &ProvenanceLayout,
        repo: &Utf8Path,
        scope: &ScopeId,
        scan_limit: usize,
        run: impl for<'c> FnOnce(&'c ReadContext) -> ReadFuture<'c, R> + Send,
    ) -> anyhow::Result<(anyhow::Result<R>, Stamp)> {
        let snapshot = match self.snapshot.take() {
            Some(snapshot) => snapshot,
            None => match ReadSnapshot::open(self.connection.pool(), scope).await {
                Ok(Some(snapshot)) => snapshot,
                Ok(None) => {
                    return Err(ReadRefusal::NoProjection {
                        database: layout.cache_db_path(),
                        because: self
                            .error
                            .as_deref()
                            .map(|error| format!(" (catch-up failed: {error})"))
                            .unwrap_or_default(),
                    }
                    .into())
                }
                Err(error) => return Err(error),
            },
        };
        let context = ReadContext::new(snapshot, repo, scan_limit);
        let result = run(&context).await;
        let stamp = stamp::seal(context, self.policy);
        Ok((result, stamp))
    }
}

pub(super) async fn run(
    layout: &ProvenanceLayout,
    scope: &provenance_core::ScopeId,
    policy: FreshnessPolicy,
) -> anyhow::Result<Freshness> {
    match policy {
        FreshnessPolicy::CatchUp => match catch_up(layout).await {
            Ok(fresh) => Ok(fresh),
            Err(error) => stored(layout, error).await,
        },
        FreshnessPolicy::AnnotateOnly => {
            let connection = open_stored_cache(layout)
                .await
                .map_err(|error| no_projection(layout, &error))?;
            if let Err(error) = ensure_current_schema(connection.pool(), layout).await {
                return Err(connection.close_reporting(error).await);
            }
            Ok(Freshness {
                connection,
                snapshot: None,
                policy: StampPolicy::AnnotateOnly,
                error: None,
            })
        }
        FreshnessPolicy::RefuseStale => super::refuse_stale::run(layout, scope).await,
    }
}

/// The guard is held across the open, so two first reads on a fresh clone
/// cannot both create the file. On a file still in DELETE mode the open
/// waits for the switch to WAL, at most the retry's deadline plus one
/// busy timeout (`cache::WalSwitchRetry`); once the file is WAL the switch
/// costs nothing.
#[rule("rule_read_holds_guard_for_freshness_only")]
async fn catch_up(layout: &ProvenanceLayout) -> anyhow::Result<Freshness> {
    let guard = publication_guard(layout).await?;
    let connection = open_cache(layout).await?;
    if let Err(error) = catch_up_with_guard(&guard, connection.pool(), layout).await {
        return Err(connection.close_reporting(error).await);
    }
    drop(guard);
    Ok(Freshness {
        connection,
        snapshot: None,
        policy: StampPolicy::CatchUp,
        error: None,
    })
}

/// The read at the stored serial after a failed freshness step. A cache
/// directory this process cannot write is the one case WAL changes: the
/// `-shm` file cannot be created, so the database opens as an immutable
/// image.
#[rule("rule_failed_freshness_answers_at_stored_serial")]
async fn stored(layout: &ProvenanceLayout, error: anyhow::Error) -> anyhow::Result<Freshness> {
    let text = format!("{error:#}");
    let connection = open_stored_cache(layout)
        .await
        .map_err(|_| ReadRefusal::NoProjection {
            database: layout.cache_db_path(),
            because: format!(" (catch-up failed: {text})"),
        })?;
    Ok(Freshness {
        connection,
        snapshot: None,
        policy: StampPolicy::CatchUpFailed,
        error: Some(text),
    })
}

/// Under `annotate_only` a database behind on migrations or validation refuses: no
/// freshness step will bring it forward. A file with no migration table
/// at all is behind too. So is a half-migrated file: a migration commits
/// and forgets the family digests, and the rebuild that refills the
/// tables runs after it, so a revision beside no digests means the tables
/// are empty and no freshness step will run to fill them. Digest rows are
/// one per scope, so a manifest that names no scope has none to lose and
/// is not read as that window.
#[rule("rule_annotate_only_refuses_a_half_migrated_projection")]
pub(super) async fn ensure_current_schema(
    pool: &sqlx::SqlitePool,
    layout: &ProvenanceLayout,
) -> anyhow::Result<()> {
    let behind = ReadRefusal::SchemaBehind {
        database: layout.cache_db_path(),
    };
    let applied = match migrations::applied_migrations(pool).await {
        Ok(applied) => applied,
        Err(error)
            if error
                .downcast_ref::<sqlx::Error>()
                .is_some_and(is_missing_table) =>
        {
            return Err(behind.into())
        }
        Err(error) => return Err(error),
    };
    if !applied
        .iter()
        .any(|id| id == migrations::LATEST_MIGRATION_ID)
    {
        return Err(behind.into());
    }
    let validation: Option<i64> =
        sqlx::query_scalar("SELECT version FROM projection_validation WHERE only_row = 1")
            .fetch_optional(pool)
            .await?;
    if validation.is_none_or(|version| version < i64::from(crate::VALIDATION_VERSION)) {
        return Err(behind.into());
    }
    let half_migrated: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM projection_revision) \
         AND NOT EXISTS (SELECT 1 FROM projection_family_digests)",
    )
    .fetch_one(pool)
    .await?;
    if half_migrated && !crate::cache::scope_ids(&layout.state_dir())?.is_empty() {
        return Err(ReadRefusal::HalfMigrated {
            database: layout.cache_db_path(),
        }
        .into());
    }
    Ok(())
}

/// Whether an error says a table this schema expects is not there.
pub fn is_missing_table(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(database) => database.message().contains("no such table"),
        _ => false,
    }
}

pub(super) fn no_projection(layout: &ProvenanceLayout, error: &anyhow::Error) -> anyhow::Error {
    ReadRefusal::NoProjection {
        database: layout.cache_db_path(),
        because: format!(" ({error:#})"),
    }
    .into()
}
