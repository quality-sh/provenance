use super::history::index::RecordIndex;
use super::history::{evidence_page, Indexing, VersionedRecord};
use crate::operations::read_policy::ReadPolicy;
use crate::operations::reader::{
    self, check_limit, Cursor, Live, Position, ReadContext, PAGE_BYTES,
};
use crate::repository::Repository;
use crate::write_error::{failure_cause, in_repository, native};
use anyhow::Context as _;
use provenance_core::protocol::{read_failure::ReadFailure, Stamped};
use provenance_core::review::{
    EvidencePage, EvidenceQuery, RecordVersion, ReviewHistoryPage, ReviewHistoryQuery,
};
use provenance_core::{NodeType, StableId};
use provenance_macros::rule;
use serde_json::Value;
use std::sync::{Arc, Mutex, PoisonError};

/// The index of the record file that one history read used, kept after
/// the read with the rows that the read made.
type Slot = Arc<Mutex<Option<RecordIndex>>>;

/// Runs one history read as [`reader::answer`] does, then keeps the index
/// rows that it made in the read database, also when the read failed. The
/// read transaction is closed by then, so the write does not wait on it.
///
/// The write is best effort: when it fails, the read gives its own answer
/// or failure. A refusal while more versions wait then says that the index
/// could not be written, so a repeat does not continue.
async fn indexed<R: Send>(
    repository: &Repository,
    policy: ReadPolicy,
    run: impl for<'c> FnOnce(&'c ReadContext, Slot) -> reader::ReadFuture<'c, R> + Send,
) -> anyhow::Result<Stamped<R>> {
    let slot = Slot::default();
    let kept = slot.clone();
    let answer = reader::answer(repository, policy, move |ctx| run(ctx, kept)).await;
    let index = slot.lock().unwrap_or_else(PoisonError::into_inner).take();
    let Some(index) = index else {
        return answer;
    };
    let saved = match repository.read_database() {
        Ok(database) => index.save(database).await,
        Err(error) => Err(error),
    };
    match (answer, saved) {
        (Err(refusal), Err(failed)) => unwritten(repository, refusal, failed),
        (answer, _) => answer,
    }
}

/// The failure of a read whose index write failed with `failed`. A refusal
/// while more versions wait gives the cause of the write, with each path
/// relative to the repository; any other failure stays as it is.
fn unwritten<R>(
    repository: &Repository,
    refusal: anyhow::Error,
    failed: anyhow::Error,
) -> anyhow::Result<Stamped<R>> {
    let Some(indexing) = refusal.downcast_ref::<Indexing>() else {
        return Err(refusal);
    };
    let located = in_repository(repository.layout().root().as_std_path(), failed);
    let cause = failure_cause(&located, native(&located));
    Err(ReadFailure::PageBudgetExceeded).context(format!(
        "versions of {} that are not in the index of record changes: {}; the history read could \
         not write the index, so a repeat does not continue: {cause}",
        indexing.path, indexing.waiting
    ))
}

pub async fn read_history(
    repository: &Repository,
    policy: ReadPolicy,
    query: ReviewHistoryQuery,
) -> anyhow::Result<Stamped<ReviewHistoryPage>> {
    check_limit(query.limit)?;
    let answer = indexed(repository, policy, move |ctx, slot| {
        Box::pin(history(ctx, query, slot))
    })
    .await?;
    crate::operations::queries::page::checked("review-history", answer)
}

async fn history(
    ctx: &ReadContext,
    query: ReviewHistoryQuery,
    slot: Slot,
) -> anyhow::Result<ReviewHistoryPage> {
    let (cursor, position) = Cursor::open(
        ctx,
        "review-history",
        &(&query.record_kind, &query.record_id, query.limit),
        query.cursor.as_deref(),
    )?;
    ctx.snapshot().bound_page_work().await?;
    let versions = versions(ctx, query.record_kind, query.record_id.clone(), slot).await?;
    let skip = usize::try_from(position.counter)?;
    let mut more = versions.len() > skip + query.limit;
    let mut entries = Vec::new();
    let mut bytes = 0;
    let mut counter = position.counter;
    // A version larger than the page bytes is alone on its page, so that
    // each page moves the cursor. The response budget check refuses it
    // when the answer is too large.
    for versioned in versions.into_iter().skip(skip).take(query.limit) {
        let size = serde_json::to_vec(&versioned.version)?.len();
        if !entries.is_empty() && bytes + size > PAGE_BYTES - 16_384 {
            more = true;
            break;
        }
        bytes += size + 1;
        entries.push(versioned.version);
        counter += 1;
    }
    Ok(ReviewHistoryPage {
        entries,
        next_cursor: more
            .then(|| {
                cursor.encode(
                    ctx,
                    Position {
                        counter,
                        ..Position::default()
                    },
                )
            })
            .transpose()?,
    })
}

/// Reads the version `entry` of one record from one read of its versions.
pub async fn read_history_entry(
    repository: &Repository,
    policy: ReadPolicy,
    kind: NodeType,
    id: StableId,
    entry: StableId,
) -> anyhow::Result<Stamped<RecordVersion>> {
    let answer = indexed(repository, policy, move |ctx, slot| {
        Box::pin(async move {
            ctx.snapshot().bound_page_work().await?;
            let versions = versions(ctx, kind, id, slot).await?;
            let found = versions
                .into_iter()
                .map(|versioned| versioned.version)
                .find(|version| version.id == entry)
                .ok_or(ReadFailure::ResourceNotFound)?;
            Ok(found)
        })
    })
    .await?;
    crate::operations::queries::page::checked("review-history-entry", answer)
}

/// Reads the versions of one record from the committed file versions, the
/// index of its record file and the saved working copy. The index, with
/// the rows that the read made, goes to `slot`.
async fn versions(
    ctx: &ReadContext,
    kind: NodeType,
    id: StableId,
    slot: Slot,
) -> anyhow::Result<Vec<VersionedRecord>> {
    // The committed versions and the working copy are outside the projection
    // stamp.
    ctx.live(Live::Diff);
    let store = ctx.live(Live::Canonical).store();
    let scope = ctx.snapshot().scope().clone();
    let records = store.history_paths(&scope, kind)?.records;
    let path = records.clone();
    let files = on_host(ctx.live(Live::Canonical).store(), move |store| async move {
        store.file_versions_of(&path).await
    })
    .await?;
    let mut index = RecordIndex::load(ctx.snapshot(), &records, &id, &files).await?;
    let (versions, index) = on_host(store, move |store| async move {
        let versions = store
            .read_record_versions(&scope, kind, &id, &files, &mut index)
            .await;
        Ok((versions, index))
    })
    .await?;
    *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(index);
    versions
}

/// Runs `work` over `store`. The host's section lock and the git adapter
/// block, so the host runs it on a thread where a blocking call is
/// permitted.
async fn on_host<T, F>(
    store: crate::state_store::StateStore,
    work: impl FnOnce(crate::state_store::StateStore) -> F + Send + 'static,
) -> anyhow::Result<T>
where
    T: Send + 'static,
    F: std::future::Future<Output = anyhow::Result<T>> + Send,
{
    #[cfg(feature = "host")]
    let done = tokio::task::spawn_blocking(move || {
        tokio::runtime::Handle::current().block_on(work(store))
    })
    .await?;
    #[cfg(not(feature = "host"))]
    let done = work(store).await;
    done
}

/// Reads a bounded span of one record version, or of the version before it.
pub async fn read_evidence(
    repository: &Repository,
    policy: ReadPolicy,
    query: EvidenceQuery,
) -> anyhow::Result<Stamped<EvidencePage>> {
    let answer = indexed(repository, policy, move |ctx, slot| {
        Box::pin(async move {
            ctx.snapshot().bound_page_work().await?;
            let versions = versions(ctx, query.record_kind, query.record_id.clone(), slot).await?;
            let index = versions
                .iter()
                .position(|versioned| versioned.version.id == query.entry_id)
                .ok_or(ReadFailure::ResourceNotFound)?;
            let selected = if query.before {
                index.checked_sub(1).ok_or(ReadFailure::ResourceNotFound)?
            } else {
                index
            };
            let record = record_of(
                ctx,
                query.record_kind,
                &query.record_id,
                &versions[selected],
            )
            .await?;
            let value = evidence_value(&versions, &record, query.field.as_deref())?;
            evidence_page(
                versions[selected].version.id.clone(),
                &value,
                query.field,
                query.offset,
            )
        })
    })
    .await?;
    crate::operations::queries::page::checked("review-evidence", answer)
}

/// The record JSON of one version: the record of the saved working copy,
/// or the record in the text of the commit of a committed version.
async fn record_of(
    ctx: &ReadContext,
    kind: NodeType,
    id: &StableId,
    versioned: &VersionedRecord,
) -> anyhow::Result<Value> {
    if let Some(record) = &versioned.record {
        return Ok(record.clone());
    }
    let store = ctx.live(Live::Canonical).store();
    let records = store.history_paths(ctx.snapshot().scope(), kind)?.records;
    let commit = versioned
        .version
        .commit
        .clone()
        .ok_or_else(|| crate::refusal::fault!("a committed record version names no commit"))?;
    let (at, id) = (commit.clone(), id.clone());
    on_host(store, move |store| async move {
        store.record_at(&records, &at, &id).await
    })
    .await?
    .ok_or_else(|| crate::refusal::fault!("the commit {commit} holds no record version"))
}

/// The value one evidence read pages: the selected record version, or one
/// field of it. A field that no version of the record holds names no
/// evidence. A field that the selected version does not hold is `null`.
#[rule("rule_evidence_read_states_absent_fields")]
fn evidence_value(
    versions: &[VersionedRecord],
    selected: &Value,
    field: Option<&str>,
) -> anyhow::Result<Value> {
    let Some(name) = field else {
        return Ok(selected.clone());
    };
    if !versions
        .iter()
        .any(|versioned| versioned.held.fields.contains_key(name))
    {
        return Err(ReadFailure::ResourceNotFound.into());
    }
    Ok(selected.get(name).cloned().unwrap_or(Value::Null))
}
