//! The freshness step a read runs before it answers.
//!
//! `catch_up` takes the publication guard, runs one catch-up pass in a
//! write of the read database, drops the guard, and reads on the same open
//! of the database. When the pass fails and the database holds a revision,
//! the read goes on at the stored serial with the policy word
//! `catch_up_failed` and the error text beside the answer. `annotate_only`
//! takes no guard and refuses an absent database. `refuse_stale` hashes
//! canonical units and refuses any digest change. The read database owns
//! the connection through its completion, so no step decides how a
//! connection closes.

use super::{ReadContext, ReadFuture, ReadRefusal, ReadSnapshot};
use crate::cache::catch_up_work;
use crate::current_schema::{self, Compatibility};
use crate::layout::ProvenanceLayout;
use crate::operations::read_policy::FreshnessPolicy;
use crate::operations::stamp;
use crate::publication::{publication_guard, PublicationGuard};
use crate::read_database::{ReadDatabase, ReadWork, Transaction};
use crate::repository::Repository;
use provenance_core::protocol::{Stamp, StampPolicy, Stamped};
use provenance_macros::rule;

pub(super) struct Freshness {
    pub(super) check: Check,
    pub(super) policy: StampPolicy,
    pub(super) error: Option<String>,
}

/// What the read checks in its transaction before it opens the snapshot.
pub(super) enum Check {
    /// A catch-up pass under this guard, in a write just before the read.
    CatchUp(PublicationGuard),
    /// Nothing: the catch-up failed.
    None,
    /// The schema, under `annotate_only`.
    Schema,
    /// The schema and the canonical digests, under `refuse_stale`. The
    /// guard, when the read has one, is held until the digests are hashed.
    Stale(Option<PublicationGuard>),
}

/// The operation of one read, and the answer that its work leaves.
struct Reading<'r, R, F> {
    repository: &'r Repository,
    scan_limit: usize,
    run: Option<F>,
    answer: Option<(anyhow::Result<R>, Stamp)>,
}

impl<R, F> Reading<'_, R, F>
where
    R: Send,
    F: for<'c> FnOnce(&'c ReadContext) -> ReadFuture<'c, R> + Send,
{
    /// The work of the read: the check, the pinned snapshot, the operation
    /// and the stamp.
    fn work<'a>(
        &'a mut self,
        check: Check,
        policy: StampPolicy,
        because: Option<&'a str>,
    ) -> ReadWork<'a> {
        Box::new(move |transaction| {
            Box::pin(async move {
                let run = self
                    .run
                    .take()
                    .ok_or_else(|| crate::refusal::fault!("a read runs its operation once"))?;
                let snapshot = open_snapshot(transaction, check, self.repository, because).await?;
                let context =
                    ReadContext::new(snapshot, self.repository.clone(), self.scan_limit, policy);
                let result = run(&context).await;
                self.answer = Some((result, stamp::seal(context, policy)));
                Ok(())
            })
        })
    }

    /// Whether the work of the read has started.
    const fn started(&self) -> bool {
        self.run.is_none()
    }
}

impl Freshness {
    /// Answers from this freshness state through the read database. This
    /// is the one completion point of a read: the snapshot opens, the body
    /// runs, the stamp seals, and the read completes its connection before
    /// the answer leaves.
    pub(super) async fn complete<R: Send>(
        self,
        repository: &Repository,
        scan_limit: usize,
        run: impl for<'c> FnOnce(&'c ReadContext) -> ReadFuture<'c, R> + Send,
    ) -> anyhow::Result<Stamped<R>> {
        let Self {
            check,
            policy,
            error,
        } = self;
        let database = repository.read_database()?;
        let mut reading = Reading {
            repository,
            scan_limit,
            run: Some(run),
            answer: None,
        };
        let (policy, error, outcome) = match check {
            Check::CatchUp(guard) => match catch_up(guard, database, &mut reading).await {
                Ok(outcome) => (policy, None, outcome),
                Err(failure) => {
                    let Self {
                        check,
                        policy,
                        error,
                    } = stored(&failure);
                    let outcome = database
                        .read(reading.work(check, policy, error.as_deref()))
                        .await;
                    (policy, error, outcome)
                }
            },
            check => {
                let outcome = database
                    .read(reading.work(check, policy, error.as_deref()))
                    .await;
                (policy, error, outcome)
            }
        };
        if let Err(failure) = outcome {
            return Err(if reading.started() {
                failure
            } else {
                unopened(repository.layout(), policy, error.as_deref(), failure)
            });
        }
        let (result, stamp) = reading.answer.ok_or_else(|| {
            crate::refusal::fault!("the read database ended a read without its work")
        })?;
        Ok(Stamped {
            result: result?,
            stamp,
            freshness_error: error,
        })
    }
}

/// Runs the check of the freshness step, then pins the snapshot.
async fn open_snapshot(
    mut transaction: Transaction,
    check: Check,
    repository: &Repository,
    because: Option<&str>,
) -> anyhow::Result<ReadSnapshot> {
    let layout = repository.layout();
    match check {
        Check::CatchUp(_) | Check::None => {}
        Check::Schema => {
            ensure_current_schema(&mut transaction, layout).await?;
        }
        Check::Stale(guard) => {
            let repeat_hash = guard.is_none();
            let snapshot =
                super::refuse_stale::check(transaction, layout, repository.scope(), repeat_hash)
                    .await;
            drop(guard);
            let snapshot = snapshot?;
            crate::test_probes::at("refuse_stale_after_hash")?;
            return Ok(snapshot);
        }
    }
    ReadSnapshot::open(transaction, repository.scope())
        .await?
        .ok_or_else(|| {
            ReadRefusal::NoProjection {
                database: layout.cache_db_path(),
                because: because
                    .map(|error| format!(" (catch-up failed: {error})"))
                    .unwrap_or_default(),
            }
            .into()
        })
}

/// The refusal of a read whose database did not open.
fn unopened(
    layout: &ProvenanceLayout,
    policy: StampPolicy,
    because: Option<&str>,
    failure: anyhow::Error,
) -> anyhow::Error {
    match (policy, because) {
        (StampPolicy::CatchUpFailed, Some(text)) => ReadRefusal::NoProjection {
            database: layout.cache_db_path(),
            because: format!(" (catch-up failed: {text})"),
        }
        .into(),
        (StampPolicy::AnnotateOnly | StampPolicy::RefuseStale, _) => {
            no_projection(layout, &failure)
        }
        _ => failure,
    }
}

pub(super) async fn run(
    repository: &Repository,
    policy: FreshnessPolicy,
) -> anyhow::Result<Freshness> {
    match policy {
        FreshnessPolicy::CatchUp => Ok(match publication_guard(repository).await {
            Ok(guard) => Freshness {
                check: Check::CatchUp(guard),
                policy: StampPolicy::CatchUp,
                error: None,
            },
            Err(error) => stored(&error),
        }),
        FreshnessPolicy::AnnotateOnly => Ok(Freshness {
            check: Check::Schema,
            policy: StampPolicy::AnnotateOnly,
            error: None,
        }),
        FreshnessPolicy::RefuseStale => super::refuse_stale::run(repository).await,
    }
}

/// The guard is held across the open, so two first reads on a fresh clone
/// cannot both create the file. On a file still in DELETE mode the open
/// waits for the switch to WAL, at most the retry's deadline plus one
/// busy timeout (`cache::WalSwitchRetry`); once the file is WAL the switch
/// costs nothing. A pass that succeeds frees the guard when it ends, and
/// the read runs on the same open of the database, which completes after
/// the read. A pass that fails frees the guard after that completion. An
/// error of the pass is the error of the freshness step; an error after it
/// is the error of the read.
#[rule("rule_read_holds_guard_for_freshness_only")]
async fn catch_up<R, F>(
    guard: PublicationGuard,
    database: &dyn ReadDatabase,
    reading: &mut Reading<'_, R, F>,
) -> Result<anyhow::Result<()>, anyhow::Error>
where
    R: Send,
    F: for<'c> FnOnce(&'c ReadContext) -> ReadFuture<'c, R> + Send,
{
    let mut guard = Some(guard);
    let mut report = None;
    let outcome = database
        .write_then_read(
            catch_up_work(&mut guard, &mut report),
            reading.work(Check::None, StampPolicy::CatchUp, None),
        )
        .await;
    drop(guard);
    match outcome {
        Err(failure) if report.is_none() => Err(failure),
        outcome => Ok(outcome),
    }
}

/// The read at the stored serial after a failed freshness step. A cache
/// directory this process cannot write is the one case WAL changes: the
/// `-shm` file cannot be created, so the database opens as an immutable
/// image.
#[rule("rule_failed_freshness_answers_at_stored_serial")]
fn stored(error: &anyhow::Error) -> Freshness {
    Freshness {
        check: Check::None,
        policy: StampPolicy::CatchUpFailed,
        error: Some(format!("{error:#}")),
    }
}

/// Under `annotate_only`, an incompatible schema or old validation refuses.
/// No freshness step will rebuild it. A revision beside no family digests
/// is also incomplete, so a read-only operation does not answer from it. Digest rows are
/// one per scope, so a manifest that names no scope has none to lose and
/// is not read as that window.
#[rule("rule_annotate_only_refuses_a_half_migrated_projection")]
pub(super) async fn ensure_current_schema(
    transaction: &mut Transaction,
    layout: &ProvenanceLayout,
) -> anyhow::Result<()> {
    let tx = transaction
        .sqlx()
        .ok_or_else(|| crate::refusal::fault!("the schema check reads the file adapter"))?;
    let behind = ReadRefusal::SchemaBehind {
        database: layout.cache_db_path(),
    };
    if current_schema::compatibility(tx).await? != Compatibility::Current {
        return Err(behind.into());
    }
    let validation: Option<i64> =
        sqlx::query_scalar("SELECT version FROM projection_validation WHERE only_row = 1")
            .fetch_optional(&mut **tx)
            .await?;
    if validation.is_none_or(|version| version < i64::from(crate::VALIDATION_VERSION)) {
        return Err(behind.into());
    }
    let half_migrated: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM projection_revision) \
         AND NOT EXISTS (SELECT 1 FROM projection_family_digests)",
    )
    .fetch_one(&mut **tx)
    .await?;
    if half_migrated && !crate::cache::scope_ids(&layout.state_dir())?.is_empty() {
        return Err(ReadRefusal::HalfMigrated {
            database: layout.cache_db_path(),
        }
        .into());
    }
    Ok(())
}

pub(super) fn no_projection(layout: &ProvenanceLayout, error: &anyhow::Error) -> anyhow::Error {
    ReadRefusal::NoProjection {
        database: layout.cache_db_path(),
        because: format!(" ({error:#})"),
    }
    .into()
}
