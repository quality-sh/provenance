use super::{nodes, served, ReadContext, ReadPolicy};
use crate::operations::reader::Cursor;
use camino::Utf8PathBuf;
use provenance_core::protocol::{
    read_failure::ReadFailure, DocumentEntry, ReadDocumentQuery, ReadDocumentResult, StampPolicy,
    Stamped,
};
use provenance_core::{NodeType, ScopeId, StableId};
use provenance_macros::rule;

/// A failed catch-up cannot establish a complete current document.
#[rule("rule_review_partial_reads_remain_explicit")]
pub async fn read_document(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    request: ReadDocumentQuery,
) -> anyhow::Result<Stamped<ReadDocumentResult>> {
    let answer = served(repo, scope, policy, move |ctx| {
        // Keep page errors until the freshness policy is checked.
        Box::pin(async move { Ok(read(ctx, request).await) })
    })
    .await?;
    if answer.stamp.policy == StampPolicy::CatchUpFailed {
        return Err(ReadFailure::DocumentCatchUpFailed.into());
    }
    super::page::checked(
        "read-document",
        Stamped {
            result: answer.result?,
            stamp: answer.stamp,
            freshness_error: answer.freshness_error,
        },
    )
}

pub(super) async fn read(
    ctx: &ReadContext,
    request: ReadDocumentQuery,
) -> anyhow::Result<ReadDocumentResult> {
    ctx.snapshot().bound_page_work().await?;
    page(ctx, request)
        .await
        .map_err(crate::operations::reader::page_error)
}

/// Retired records remain references and cannot supply member expansion.
#[rule("rule_review_retired_records_do_not_expand_active_document")]
async fn page(ctx: &ReadContext, request: ReadDocumentQuery) -> anyhow::Result<ReadDocumentResult> {
    use crate::operations::reader::{PAGE_BYTES, RECORD_BYTES};
    request
        .validate()
        .map_err(provenance_core::protocol::QueryValidation::into_native)?;
    let (cursor, mut position) = Cursor::open(
        ctx,
        "read-document",
        &(&request.id, request.limit),
        request.cursor.as_deref(),
    )?;
    let root = nodes::page_node(ctx.snapshot(), NodeType::Requirement, &request.id)
        .await?
        .ok_or(ReadFailure::DocumentRootMissing)?;
    if root.retired() {
        return Err(ReadFailure::DocumentRootRetired.into());
    }
    let keys = ctx
        .snapshot()
        .document_keys(&request.id, &position, request.limit + 1)
        .await?;
    let mut entries = Vec::new();
    let mut bytes = 0;
    let mut has_more = keys.len() > request.limit;
    for (kind, key) in keys.into_iter().take(request.limit) {
        if entries.len() == request.limit {
            has_more = true;
            break;
        }
        let entry = match key.stage {
            2 => ctx
                .snapshot()
                .page_thread(&key.id)
                .await?
                .map(|thread| DocumentEntry::Thread { thread }),
            3 => ctx
                .snapshot()
                .page_message(&key.id)
                .await?
                .map(|message| DocumentEntry::Message { message }),
            stage => nodes::page_node(ctx.snapshot(), NodeType::parse(&kind)?, &key.id)
                .await?
                .map(|node| {
                    if stage == 0 {
                        DocumentEntry::Member { node }
                    } else {
                        DocumentEntry::Reference { node }
                    }
                }),
        };
        if let Some(entry) = entry {
            let size = serde_json::to_vec(&entry)?.len();
            if size > RECORD_BYTES {
                return Err(ReadFailure::PageRecordTooLarge.into());
            }
            if bytes + size > PAGE_BYTES {
                has_more = true;
                break;
            }
            bytes += size;
            entries.push(entry);
        }
        position = key;
    }
    Ok(ReadDocumentResult {
        root_id: StableId::new(request.id)?,
        limit: request.limit,
        next_cursor: if has_more {
            Some(cursor.encode(ctx, position)?)
        } else {
            None
        },
        has_more,
        entries,
    })
}
