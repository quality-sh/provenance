//! The freshness step a read runs before it answers.
//!
//! `catch_up` takes the publication guard, runs one catch-up pass in a
//! write of the read database, drops the guard, and reads on the same open
//! of the database. When the pass fails and the database holds a revision,
//! the read goes on at the stored serial with the policy word
//! `catch_up_failed` and the error text beside the answer; a read that
//! needs a current answer refuses with that error instead. `annotate_only`
//! takes no guard and refuses an absent database. `refuse_stale` hashes
//! canonical units and refuses any digest change. The read database owns
//! the connection through its completion, so no step decides how a
//! connection closes.
//!
//! Without `host` a read takes no publication guard and never writes the
//! read database: the embedder catches the database up after each of its
//! publishes ([`catch_up_in_publish`]). A `catch_up` read then answers as
//! an `annotate_only` read does, and its stamp says `annotate_only`.
//!
//! [`catch_up_in_publish`]: crate::cache::catch_up_in_publish

use super::{CatchUpFailed, ReadContext, ReadFuture, ReadRefusal, ReadSnapshot};
#[cfg(feature = "host")]
use crate::cache::catch_up_work;
use crate::current_schema::{self, Compatibility};
use crate::layout::ProvenanceLayout;
use crate::operations::read_policy::FreshnessPolicy;
use crate::operations::stamp;
#[cfg(feature = "host")]
use crate::publication::publication_guard;
use crate::publication::PublicationGuard;
use crate::read_database::{ReadDatabase, ReadWork, SqlColumns, Transaction, Unopened};
use crate::repository::Repository;
use provenance_core::protocol::{Stamp, StampPolicy, Stamped};
use provenance_macros::rule;

pub(super) struct Freshness {
    pub(super) check: Check,
    pub(super) policy: StampPolicy,
    /// The error of a failed catch-up.
    pub(super) failure: Option<anyhow::Error>,
}

/// A read that completed: the result of its operation, its stamp, and the
/// error of a failed catch-up when the read answered at the stored serial.
pub(super) struct Completed<R> {
    result: anyhow::Result<R>,
    stamp: Stamp,
    failure: Option<anyhow::Error>,
}

impl<R> Completed<R> {
    /// The answer, with the text of a failed catch-up beside it.
    pub(super) fn answer(self) -> anyhow::Result<Stamped<R>> {
        Ok(Stamped {
            result: self.result?,
            stamp: self.stamp,
            freshness_error: self.failure.map(|failure| format!("{failure:#}")),
        })
    }

    /// The answer of a read that needs a current answer. A failed catch-up
    /// refuses with [`CatchUpFailed`], before an error of the operation.
    pub(super) fn current(mut self) -> anyhow::Result<Stamped<R>> {
        if let Some(failure) = self.failure.take() {
            return Err(CatchUpFailed(failure).into());
        }
        self.answer()
    }
}

/// What the read checks in its transaction before it opens the snapshot.
pub(super) enum Check {
    /// A catch-up pass under this guard, in a write just before the read.
    #[cfg(feature = "host")]
    CatchUp(PublicationGuard),
    /// Nothing: the catch-up failed.
    #[cfg(feature = "host")]
    None,
    /// The schema, under `annotate_only`.
    Schema,
    /// The schema and the canonical digests, under `refuse_stale`, hashed
    /// `passes` times. The guard, when the read has one, is held until the
    /// digests are hashed.
    Stale {
        guard: Option<PublicationGuard>,
        passes: usize,
    },
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
    ) -> anyhow::Result<Completed<R>> {
        let database = repository.read_database()?;
        let mut reading = Reading {
            repository,
            scan_limit,
            run: Some(run),
            answer: None,
        };
        let failure = match self.check {
            #[cfg(feature = "host")]
            Check::CatchUp(guard) => match catch_up(guard, database, &mut reading).await {
                Ok(outcome) => {
                    outcome.map_err(|failure| {
                        unopened(repository.layout(), self.policy, None, failure)
                    })?;
                    None
                }
                Err(error) => stored(error).read(database, &mut reading).await?,
            },
            check => Self { check, ..self }.read(database, &mut reading).await?,
        };
        let (result, stamp) = reading.answer.ok_or_else(|| {
            crate::refusal::fault!("the read database ended a read without its work")
        })?;
        Ok(Completed {
            result,
            stamp,
            failure,
        })
    }

    /// Runs the work of `reading` on the snapshot that the check opens, and
    /// gives back the error of a failed catch-up. A database that did not
    /// open refuses as [`unopened`] says.
    async fn read<R, F>(
        self,
        database: &dyn ReadDatabase,
        reading: &mut Reading<'_, R, F>,
    ) -> anyhow::Result<Option<anyhow::Error>>
    where
        R: Send,
        F: for<'c> FnOnce(&'c ReadContext) -> ReadFuture<'c, R> + Send,
    {
        let Self {
            check,
            policy,
            failure,
        } = self;
        let repository = reading.repository;
        let because = failure
            .as_ref()
            .map(|failure| format!("catch-up failed: {}", cause(repository.layout(), failure)));
        if let Err(outcome) = database
            .read(reading.work(check, policy, because.as_deref()))
            .await
        {
            return Err(unopened(
                repository.layout(),
                policy,
                because.as_deref(),
                outcome,
            ));
        }
        Ok(failure)
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
        #[cfg(feature = "host")]
        Check::CatchUp(_) | Check::None => {}
        Check::Schema => {
            ensure_current_schema(&mut transaction, layout).await?;
        }
        Check::Stale { guard, passes } => {
            let snapshot = super::refuse_stale::check(transaction, repository, passes).await;
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
                cause: because.map(str::to_owned),
            }
            .into()
        })
}

/// The refusal of a read whose database did not open. Any other failure,
/// such as a transaction that did not begin, is the failure of the read.
fn unopened(
    layout: &ProvenanceLayout,
    policy: StampPolicy,
    because: Option<&str>,
    failure: anyhow::Error,
) -> anyhow::Error {
    let cause = match Unopened::cause(failure) {
        Ok(cause) => cause,
        Err(failure) => return failure,
    };
    match (policy, because) {
        (StampPolicy::CatchUpFailed, Some(text)) => ReadRefusal::NoProjection {
            database: layout.cache_db_path(),
            cause: Some(text.to_owned()),
        }
        .into(),
        (StampPolicy::AnnotateOnly | StampPolicy::RefuseStale, _) => no_projection(layout, &cause),
        _ => cause,
    }
}

pub(super) async fn run(
    repository: &Repository,
    policy: FreshnessPolicy,
) -> anyhow::Result<Freshness> {
    match policy {
        FreshnessPolicy::CatchUp => {
            // A read whose pass finds the catch-up claim held, as by a pass
            // of `catch_up_in_publish`, answers as a read whose catch-up
            // failed.
            #[cfg(feature = "host")]
            let freshness = match publication_guard(repository)
                .await
                .and_then(|guard| guard.for_pass().map_err(anyhow::Error::from))
            {
                Ok(guard) => Freshness {
                    check: Check::CatchUp(guard),
                    policy: StampPolicy::CatchUp,
                    failure: None,
                },
                Err(error) => stored(error),
            };
            // No catch-up runs without `host`, so the read is an
            // `annotate_only` read and its stamp says so.
            #[cfg(not(feature = "host"))]
            let freshness = annotate_only();
            Ok(freshness)
        }
        FreshnessPolicy::AnnotateOnly => Ok(annotate_only()),
        FreshnessPolicy::RefuseStale => super::refuse_stale::run(repository).await,
    }
}

/// A read at the stored serial with no freshness step but the schema check.
const fn annotate_only() -> Freshness {
    Freshness {
        check: Check::Schema,
        policy: StampPolicy::AnnotateOnly,
        failure: None,
    }
}

/// The guard is held across the open, so two first reads on a fresh clone
/// cannot both create the file. On a file still in DELETE mode the open
/// waits for the switch to WAL, at most the retry's deadline plus one
/// busy timeout (`cache::WalSwitchRetry`); once the file is WAL the switch
/// costs nothing. A pass that succeeds frees the guard when it ends, and
/// the read runs on the same open of the database, which completes after
/// the read. A pass that fails frees the guard after that completion. A
/// failure of the write of the pass, its commit included, is the error of
/// the freshness step, even when the pass gave its report; an error after
/// it is the error of the read.
#[rule("rule_read_holds_guard_for_freshness_only")]
#[cfg(feature = "host")]
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
            catch_up_work(&mut guard, &mut report, database.max_bound_values()),
            reading.work(Check::None, StampPolicy::CatchUp, None),
        )
        .await;
    drop(guard);
    outcome
}

/// The read at the stored serial after a failed freshness step. A cache
/// directory this process cannot write is the one case WAL changes: the
/// `-shm` file cannot be created, so the database opens as an immutable
/// image.
#[rule("rule_failed_freshness_answers_at_stored_serial")]
#[cfg(feature = "host")]
const fn stored(error: anyhow::Error) -> Freshness {
    Freshness {
        check: Check::None,
        policy: StampPolicy::CatchUpFailed,
        failure: Some(error),
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
    let behind = ReadRefusal::SchemaBehind {
        database: layout.cache_db_path(),
    };
    if current_schema::compatibility(transaction).await? != Compatibility::Current {
        return Err(behind.into());
    }
    let validation = crate::cache::stored_validation_version(transaction).await?;
    if validation.is_none_or(|version| version < i64::from(crate::VALIDATION_VERSION)) {
        return Err(behind.into());
    }
    let rows = transaction
        .fetch(
            "SELECT EXISTS (SELECT 1 FROM projection_revision) \
             AND NOT EXISTS (SELECT 1 FROM projection_family_digests)",
            Vec::new(),
        )
        .await?;
    let half_migrated = super::one_row(&rows)?.integer(0)? != 0;
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
        cause: Some(cause(layout, error)),
    }
    .into()
}

/// The cause of a refusal with no stored revision: the display chain of
/// `error`, with each path below the repository of `layout` relative to it
/// and each other absolute path only its file name.
fn cause(layout: &ProvenanceLayout, error: &anyhow::Error) -> String {
    crate::write_error::repository_cause(layout.root().as_std_path(), error)
}
