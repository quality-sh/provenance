use super::history::{evidence_page, VersionedRecord};
use crate::operations::read_policy::ReadPolicy;
use crate::operations::reader::{self, Cursor, Live, Position, ReadContext, PAGE_BYTES};
use camino::Utf8Path;
use provenance_core::protocol::{read_failure::ReadFailure, Stamped};
use provenance_core::review::{EvidencePage, EvidenceQuery, ReviewHistoryPage, ReviewHistoryQuery};
use provenance_core::{NodeType, StableId};

pub async fn read_history(
    repo: &Utf8Path,
    scope: &provenance_core::ScopeId,
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
    let (cursor, position) = Cursor::open(
        ctx,
        "review-history",
        &(&query.record_kind, &query.record_id, query.limit),
        query.cursor.as_deref(),
    )?;
    ctx.snapshot().bound_page_work().await?;
    let versions = versions(ctx, query.record_kind, query.record_id.clone()).await?;
    let skip = usize::try_from(position.counter)?;
    let mut more = versions.len() > skip + query.limit;
    let mut entries = Vec::new();
    let mut bytes = 0;
    let mut counter = position.counter;
    for versioned in versions.into_iter().skip(skip).take(query.limit) {
        let size = serde_json::to_vec(&versioned.version)?.len();
        if bytes + size > PAGE_BYTES - 16_384 {
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

/// Reads the versions of one record from Git and the saved working copy.
async fn versions(
    ctx: &ReadContext,
    kind: NodeType,
    id: StableId,
) -> anyhow::Result<Vec<VersionedRecord>> {
    // Git history and the working copy are outside the projection stamp.
    ctx.live(Live::Diff);
    let store = ctx.live(Live::Canonical).store();
    let scope = ctx.snapshot().scope().clone();
    tokio::task::spawn_blocking(move || store.record_versions(&scope, kind, &id)).await?
}

/// Reads a bounded span of one record version, or of the version before it.
pub async fn read_evidence(
    repo: &Utf8Path,
    scope: &provenance_core::ScopeId,
    policy: ReadPolicy,
    query: EvidenceQuery,
) -> anyhow::Result<Stamped<EvidencePage>> {
    let answer = reader::answer(repo, scope, policy, move |ctx| {
        Box::pin(async move {
            ctx.snapshot().bound_page_work().await?;
            let versions = versions(ctx, query.record_kind, query.record_id.clone()).await?;
            let index = versions
                .iter()
                .position(|versioned| versioned.version.id == query.entry_id)
                .ok_or(ReadFailure::ResourceNotFound)?;
            let selected = if query.before {
                index
                    .checked_sub(1)
                    .ok_or_else(|| anyhow::anyhow!("this version has no Before version"))?
            } else {
                index
            };
            let versioned = &versions[selected];
            evidence_page(
                versioned.version.id.clone(),
                &versioned.record,
                query.field,
                query.offset,
            )
        })
    })
    .await?;
    crate::operations::queries::page::checked("review-evidence", answer)
}
