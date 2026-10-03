use super::{nodes, served, ReadContext, ReadPolicy};
use crate::operations::reader::Cursor;
use camino::Utf8PathBuf;
use provenance_core::protocol::{
    read_failure::ReadFailure, DocumentEntry, DocumentReviewTotals, ReadDocumentQuery,
    ReadDocumentResult, StampPolicy, Stamped,
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
    read_document_answer(repo, scope, policy, request)
        .await
        .and_then(|answer| super::page::checked("read-document", answer))
}

pub async fn read_document_answer(
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
    Ok(Stamped {
        result: answer.result?,
        stamp: answer.stamp,
        freshness_error: answer.freshness_error,
    })
}

/// Finds each Requirement whose canonical review document contains one record.
pub async fn containing_review_documents(
    repo: Option<Utf8PathBuf>,
    scope: &ScopeId,
    policy: ReadPolicy,
    kind: NodeType,
    id: &StableId,
) -> anyhow::Result<Stamped<Vec<StableId>>> {
    let id = id.clone();
    served(repo, scope, policy, move |ctx| {
        Box::pin(async move { ctx.snapshot().containing_document_roots(kind, &id).await })
    })
    .await
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

async fn review_keys(
    ctx: &ReadContext,
    root: &str,
    exclude_terminal: bool,
) -> anyhow::Result<Vec<(NodeType, StableId, bool)>> {
    ctx.snapshot()
        .document_keys(
            root,
            &crate::operations::reader::Position::default(),
            8193,
            exclude_terminal,
        )
        .await?
        .into_iter()
        .filter(|(_, key)| key.stage < 2)
        .map(|(kind, key)| {
            Ok((
                NodeType::parse(&kind)?,
                StableId::new(key.id)?,
                key.stage == 0,
            ))
        })
        .collect()
}

async fn page(ctx: &ReadContext, request: ReadDocumentQuery) -> anyhow::Result<ReadDocumentResult> {
    use crate::operations::reader::PAGE_BYTES;
    request
        .validate()
        .map_err(provenance_core::protocol::QueryValidation::into_native)?;
    nodes::page_node(ctx.snapshot(), NodeType::Requirement, &request.id)
        .await?
        .ok_or(ReadFailure::DocumentRootMissing)?;
    let review_keys = review_keys(ctx, &request.id, request.exclude_terminal).await?;
    let review_records = review_keys
        .iter()
        .map(|(kind, id, _)| (*kind, id.clone()))
        .collect::<Vec<_>>();
    let review_state = ctx
        .live(crate::operations::reader::Live::Canonical)
        .store()
        .document_review_state(ctx.snapshot().scope(), &review_records)?;
    let mut review_totals = DocumentReviewTotals::default();
    for (kind, id, is_member) in &review_keys {
        if *is_member {
            review_totals.include(&review_state.summary(*kind, id));
        }
    }
    let (cursor, mut position) = Cursor::open_live(
        ctx,
        "read-document",
        &(&request.id, request.limit, request.exclude_terminal),
        request.cursor.as_deref(),
        &review_state.digest,
    )?;
    let keys = ctx
        .snapshot()
        .document_keys(
            &request.id,
            &position,
            request.limit + 1,
            request.exclude_terminal,
        )
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
                    let review = review_state.summary(node.node_type(), node.id());
                    if stage == 0 {
                        DocumentEntry::Member { node, review }
                    } else {
                        DocumentEntry::Reference { node, review }
                    }
                }),
        };
        if let Some(entry) = entry {
            let size = serde_json::to_vec(&entry)?.len();
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
        review_totals,
        entries,
    })
}
