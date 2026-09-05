//! The freshness step a read runs before it answers.
//!
//! `catch_up` takes the publication guard, opens the pool inside it, runs
//! one catch-up pass, and drops the guard. When that step fails and the
//! database holds a revision, the read goes on at the stored serial with
//! the policy word `catch_up_failed` and the error text beside the answer.
//! `annotate_only` takes no guard and refuses an absent database.
//! `refuse_stale` is reserved and not implemented yet.

use super::ReadRefusal;
use crate::cache::{catch_up_with_guard, open_cache, open_existing_cache, open_immutable_cache};
use crate::layout::ProvenanceLayout;
use crate::migrations;
use crate::operations::read_policy::FreshnessPolicy;
use crate::publication::publication_guard;
use provenance_core::protocol::StampPolicy;
use provenance_macros::rule;
use sqlx::SqlitePool;

pub(super) struct Freshness {
    pub pool: SqlitePool,
    pub policy: StampPolicy,
    pub error: Option<String>,
}

pub(super) async fn run(
    layout: &ProvenanceLayout,
    policy: FreshnessPolicy,
) -> anyhow::Result<Freshness> {
    match policy {
        FreshnessPolicy::CatchUp => match catch_up(layout).await {
            Ok(pool) => Ok(Freshness {
                pool,
                policy: StampPolicy::CatchUp,
                error: None,
            }),
            Err(error) => stored(layout, error).await,
        },
        FreshnessPolicy::AnnotateOnly => {
            let pool = open_existing_cache(layout)
                .await
                .map_err(|error| no_projection(layout, &error))?;
            if let Err(error) = ensure_current_schema(&pool, layout).await {
                pool.close().await;
                return Err(error);
            }
            Ok(Freshness {
                pool,
                policy: StampPolicy::AnnotateOnly,
                error: None,
            })
        }
        FreshnessPolicy::RefuseStale => Err(ReadRefusal::RefuseStaleUnimplemented.into()),
    }
}

/// The guard is held across the open, so two first reads on a fresh clone
/// cannot both create the file. On a file still in DELETE mode the open
/// waits for the switch to WAL, at most the retry's deadline plus one
/// busy timeout (`cache::WalSwitchRetry`); once the file is WAL the switch
/// costs nothing.
async fn catch_up(layout: &ProvenanceLayout) -> anyhow::Result<SqlitePool> {
    let guard = publication_guard(layout).await?;
    let pool = open_cache(layout).await?;
    if let Err(error) = catch_up_with_guard(&guard, &pool, layout).await {
        pool.close().await;
        return Err(error);
    }
    drop(guard);
    Ok(pool)
}

/// The read at the stored serial after a failed freshness step. A cache
/// directory this process cannot write is the one case WAL changes: the
/// `-shm` file cannot be created, so the database opens as an immutable
/// image.
async fn stored(layout: &ProvenanceLayout, error: anyhow::Error) -> anyhow::Result<Freshness> {
    let text = format!("{error:#}");
    let pool = match open_existing_cache(layout).await {
        Ok(pool) => pool,
        Err(_) => open_immutable_cache(layout)
            .await
            .map_err(|_| ReadRefusal::NoProjection {
                database: layout.cache_db_path(),
                because: format!(" (catch-up failed: {text})"),
            })?,
    };
    Ok(Freshness {
        pool,
        policy: StampPolicy::CatchUpFailed,
        error: Some(text),
    })
}

/// Under `annotate_only` a database behind on migrations refuses: no
/// freshness step will bring it forward. A file with no migration table
/// at all is behind too. So is a half-migrated file: a migration commits
/// and forgets the family digests, and the rebuild that refills the
/// tables runs after it, so a revision beside no digests means the tables
/// are empty and no freshness step will run to fill them.
#[rule("rule_annotate_only_refuses_a_half_migrated_projection")]
async fn ensure_current_schema(pool: &SqlitePool, layout: &ProvenanceLayout) -> anyhow::Result<()> {
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
    let half_migrated: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM projection_revision) \
         AND NOT EXISTS (SELECT 1 FROM projection_family_digests)",
    )
    .fetch_one(pool)
    .await?;
    if half_migrated {
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

fn no_projection(layout: &ProvenanceLayout, error: &anyhow::Error) -> anyhow::Error {
    ReadRefusal::NoProjection {
        database: layout.cache_db_path(),
        because: format!(" ({error:#})"),
    }
    .into()
}
