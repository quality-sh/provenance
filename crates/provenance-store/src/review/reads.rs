use super::journal;
use super::snapshot::evidence;
use crate::operations::read_policy::ReadPolicy;
use crate::operations::reader::{
    self, Cursor, Live, Position, ReadContext, ReadSnapshot, PAGE_BYTES, RECORD_BYTES,
};
use crate::state_store::StateStore;
use camino::Utf8Path;
use provenance_core::protocol::{read_failure::ReadFailure, Stamped};
use provenance_core::review::{
    EvidencePage, EvidenceQuery, ReviewEntry, ReviewHistoryPage, ReviewHistoryQuery,
};
use provenance_core::{ScopeId, StableId};

pub async fn read_history(
    repo: &Utf8Path,
    scope: &ScopeId,
    policy: ReadPolicy,
    query: ReviewHistoryQuery,
) -> anyhow::Result<Stamped<ReviewHistoryPage>> {
    anyhow::ensure!(
        (1..=200).contains(&query.limit),
        "review history limit must be between 1 and 200"
    );
    let answer = reader::answer(repo, scope, policy, move |ctx| {
        Box::pin(history(ctx, query))
    })
    .await?;
    crate::operations::queries::page::checked("review-history", answer)
}

async fn history(
    ctx: &ReadContext,
    query: ReviewHistoryQuery,
) -> anyhow::Result<ReviewHistoryPage> {
    let (cursor, mut position) = Cursor::open(
        ctx,
        "review-history",
        &(&query.requirement_id, query.limit),
        query.cursor.as_deref(),
    )?;
    ctx.snapshot().bound_page_work().await?;
    ctx.snapshot().attest("review_journal");
    let mut tx = ctx.snapshot().connection().await;
    let keys: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT id, sequence, length(CAST(payload AS BLOB)) FROM review_journal WHERE scope_id = ? AND requirement_id = ? AND sequence > ? ORDER BY sequence LIMIT ?"
    ).bind(ctx.snapshot().scope().as_str()).bind(query.requirement_id.as_str()).bind(position.counter)
        .bind(i64::try_from(query.limit + 1)?).fetch_all(&mut **tx).await.map_err(anyhow::Error::from).map_err(reader::page_error)?;
    drop(tx);
    let mut more = keys.len() > query.limit;
    let mut entries = Vec::new();
    let mut bytes = 0;
    for (id, sequence, size) in keys.into_iter().take(query.limit) {
        if size > i64::try_from(RECORD_BYTES)? {
            return Err(ReadFailure::PageRecordTooLarge.into());
        }
        if bytes + usize::try_from(size)? > PAGE_BYTES - 16_384 {
            more = true;
            break;
        }
        let entry = ctx
            .snapshot()
            .review_entry(&query.requirement_id, &id)
            .await?;
        bytes += serde_json::to_vec(&entry)?.len() + 1;
        entries.push(entry);
        position = Position {
            counter: sequence,
            ..Position::default()
        };
    }
    Ok(ReviewHistoryPage {
        entries,
        next_cursor: more.then(|| cursor.encode(ctx, position)).transpose()?,
    })
}

impl ReadSnapshot {
    async fn review_entry(&self, requirement: &StableId, id: &str) -> anyhow::Result<ReviewEntry> {
        self.attest("review_journal");
        let mut tx = self.connection().await;
        let row: Option<(i64, Option<String>)> = sqlx::query_as(
            "SELECT length(CAST(payload AS BLOB)), CASE WHEN length(CAST(payload AS BLOB)) <= ? THEN payload END FROM review_journal WHERE scope_id = ? AND requirement_id = ? AND id = ?"
        ).bind(i64::try_from(RECORD_BYTES)?).bind(self.scope().as_str()).bind(requirement.as_str()).bind(id)
            .fetch_optional(&mut **tx).await?;
        drop(tx);
        let (size, payload) =
            row.ok_or_else(|| anyhow::anyhow!("review entry does not exist at this address"))?;
        if size > i64::try_from(RECORD_BYTES)? {
            return Err(ReadFailure::PageRecordTooLarge.into());
        }
        Ok(serde_json::from_str(&payload.unwrap())?)
    }
}

/// Reads a bounded span from the immutable snapshot named by a pinned journal row.
pub async fn read_evidence(
    repo: &Utf8Path,
    scope: &ScopeId,
    policy: ReadPolicy,
    query: EvidenceQuery,
) -> anyhow::Result<Stamped<EvidencePage>> {
    let answer = reader::answer(repo, scope, policy, move |ctx| {
        Box::pin(async move {
            ctx.snapshot().bound_page_work().await?;
            let entry = ctx
                .snapshot()
                .review_entry(&query.requirement_id, query.entry_id.as_str())
                .await?;
            let reference = if query.before {
                entry
                    .before
                    .ok_or_else(|| anyhow::anyhow!("this change has no Before snapshot"))?
            } else {
                entry.after
            };
            let store = ctx.live(Live::Canonical).store();
            let scope = ctx.snapshot().scope().clone();
            tokio::task::spawn_blocking(move || {
                store.with_repository_publication(|| {
                    evidence(&store, &scope, reference, query.field, query.offset)
                })
            })
            .await?
        })
    })
    .await?;
    crate::operations::queries::page::checked("review-evidence", answer)
}

impl StateStore {
    /// An absent receipt is authoritative only after publication recovery and owner checks.
    pub fn requirement_save_receipt(
        &self,
        scope: &ScopeId,
        requirement: &StableId,
        request: &StableId,
        actor: &str,
        owner: Option<&str>,
    ) -> anyhow::Result<Option<ReviewEntry>> {
        self.with_repository_publication(|| {
            let record = self.requirement(scope, requirement)?;
            super::owner_matches(&record, owner)?;
            let path = journal::entry_path(&self.layout, scope, request);
            if !path.try_exists()? {
                return Ok(None);
            }
            let entry = journal::read_entry(&path)?;
            anyhow::ensure!(
                entry.request_id == *request
                    && entry.scope_id == *scope
                    && entry.requirement_id == *requirement
                    && entry.actor == actor,
                "receipt identity mismatch"
            );
            Ok(Some(entry))
        })
    }
}
