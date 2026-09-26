use crate::operations::{
    read_policy::ReadPolicy,
    reader::{self, Cursor, Position, ReadContext, PAGE_BYTES, RECORD_BYTES},
};
use camino::Utf8Path;
use provenance_core::{
    protocol::{read_failure::ReadFailure, Stamped},
    threads::{DiscussionEntry, DiscussionGroup, DiscussionPage, DiscussionQuery},
    ScopeId, StableId, ThreadParent,
};

pub async fn read_discussions(
    repo: &Utf8Path,
    scope: &ScopeId,
    policy: ReadPolicy,
    query: DiscussionQuery,
) -> anyhow::Result<Stamped<DiscussionPage>> {
    check_limit(query.limit)?;
    let answer =
        reader::answer(repo, scope, policy, move |ctx| Box::pin(groups(ctx, query))).await?;
    crate::operations::queries::page::checked("review-discussions", answer)
}

pub async fn read_discussion(
    repo: &Utf8Path,
    scope: &ScopeId,
    policy: ReadPolicy,
    parent: ThreadParent,
    discussion_id: StableId,
) -> anyhow::Result<Stamped<DiscussionGroup>> {
    reader::answer(repo, scope, policy, move |ctx| {
        Box::pin(group(ctx, parent, discussion_id))
    })
    .await
}

pub(super) fn check_limit(limit: usize) -> anyhow::Result<()> {
    anyhow::ensure!(
        (1..=200).contains(&limit),
        "Discussion page limit must be between 1 and 200"
    );
    Ok(())
}

pub(super) async fn check_parent(ctx: &ReadContext, parent: &ThreadParent) -> anyhow::Result<()> {
    let table = super::discussion_state::parent_table(parent.node_type)
        .ok_or_else(|| anyhow::anyhow!("thread parent kind does not take Discussions"))?;
    ctx.snapshot().attest(table);
    let mut tx = ctx.snapshot().connection().await;
    let exists: bool = sqlx::query_scalar(&format!(
        "SELECT EXISTS(SELECT 1 FROM {table} WHERE scope_id = ? AND id = ?)"
    ))
    .bind(ctx.snapshot().scope().as_str())
    .bind(parent.node_id.as_str())
    .fetch_one(&mut **tx)
    .await?;
    drop(tx);
    if !exists {
        return Err(ReadFailure::ResourceNotFound.into());
    }
    Ok(())
}

async fn groups(ctx: &ReadContext, query: DiscussionQuery) -> anyhow::Result<DiscussionPage> {
    let (cursor, mut position) = Cursor::open(
        ctx,
        "review-discussions",
        &(&query.parent, query.limit),
        query.cursor.as_deref(),
    )?;
    ctx.snapshot().bound_page_work().await?;
    check_parent(ctx, &query.parent).await?;
    for family in ["review_journal", "threads", "messages"] {
        ctx.snapshot().attest(family);
    }
    let mut entries = Vec::new();
    let mut bytes = 0;
    if position.stage == 0 {
        let mut tx = ctx.snapshot().connection().await;
        let keys: Vec<(String, String, i64, String)> = sqlx::query_as(
            "SELECT j.discussion_id, j.id, length(CAST(j.payload AS BLOB)), t.status FROM review_journal j JOIN threads t ON t.scope_id=j.scope_id AND t.id=j.thread_id WHERE j.scope_id=? AND j.parent_type=? AND j.parent_id=? AND j.discussion_id>? AND NOT EXISTS(SELECT 1 FROM review_journal n WHERE n.scope_id=j.scope_id AND n.discussion_id=j.discussion_id AND n.version>j.version) ORDER BY j.discussion_id LIMIT ?"
        ).bind(ctx.snapshot().scope().as_str())
        .bind(super::discussion_state::discussion_kind_word(query.parent.node_type))
        .bind(query.parent.node_id.as_str()).bind(&position.id)
            .bind(i64::try_from(query.limit + 1)?).fetch_all(&mut **tx).await.map_err(anyhow::Error::from).map_err(reader::page_error)?;
        drop(tx);
        for (discussion, id, size, status) in keys {
            if entries.len() == query.limit || bytes + usize::try_from(size)? > PAGE_BYTES - 16_384
            {
                return Ok(DiscussionPage {
                    entries,
                    next_cursor: Some(cursor.encode(ctx, position)?),
                });
            }
            if size > i64::try_from(RECORD_BYTES)? {
                return Err(ReadFailure::PageRecordTooLarge.into());
            }
            let mut tx = ctx.snapshot().connection().await;
            let payload: String =
                sqlx::query_scalar("SELECT payload FROM review_journal WHERE scope_id=? AND id=?")
                    .bind(ctx.snapshot().scope().as_str())
                    .bind(id)
                    .fetch_one(&mut **tx)
                    .await?;
            drop(tx);
            let group = DiscussionGroup::Addressed {
                discussion: Box::new(serde_json::from_str::<DiscussionEntry>(&payload)?),
                container_status: serde_json::from_value(serde_json::Value::String(status))?,
            };
            let size = serde_json::to_vec(&group)?.len();
            if size > RECORD_BYTES {
                return Err(ReadFailure::PageRecordTooLarge.into());
            }
            bytes += size + 1;
            entries.push(group);
            position.id = discussion;
        }
        position = Position {
            stage: 1,
            ..Position::default()
        };
    }
    let mut tx = ctx.snapshot().connection().await;
    let keys: Vec<(String,String)> = sqlx::query_as("SELECT t.id,t.status FROM threads t WHERE t.scope_id=? AND t.parent_type=? AND t.parent_id=? AND t.id>? AND EXISTS(SELECT 1 FROM messages m WHERE m.scope_id=t.scope_id AND m.thread_id=t.id AND NOT EXISTS(SELECT 1 FROM review_journal j WHERE j.scope_id=m.scope_id AND j.message_id=m.id)) ORDER BY t.id LIMIT ?")
        .bind(ctx.snapshot().scope().as_str())
        .bind(super::discussion_state::discussion_kind_word(query.parent.node_type))
        .bind(query.parent.node_id.as_str()).bind(&position.id)
        .bind(i64::try_from(query.limit + 1)?).fetch_all(&mut **tx).await.map_err(anyhow::Error::from).map_err(reader::page_error)?;
    drop(tx);
    for (id, status) in keys {
        let group = DiscussionGroup::Legacy {
            thread_id: StableId::new(id.clone())?,
            parent: query.parent.clone(),
            container_status: serde_json::from_value(serde_json::Value::String(status))?,
        };
        let size = serde_json::to_vec(&group)?.len();
        if size > RECORD_BYTES {
            return Err(ReadFailure::PageRecordTooLarge.into());
        }
        if entries.len() == query.limit || bytes + size > PAGE_BYTES - 16_384 {
            return Ok(DiscussionPage {
                entries,
                next_cursor: Some(cursor.encode(ctx, position)?),
            });
        }
        bytes += size + 1;
        entries.push(group);
        position.id = id;
    }
    Ok(DiscussionPage {
        entries,
        next_cursor: None,
    })
}

pub(super) async fn group(
    ctx: &ReadContext,
    parent: ThreadParent,
    discussion_id: StableId,
) -> anyhow::Result<DiscussionGroup> {
    ctx.snapshot().bound_page_work().await?;
    check_parent(ctx, &parent).await?;
    for family in ["review_journal", "threads"] {
        ctx.snapshot().attest(family);
    }
    let mut tx = ctx.snapshot().connection().await;
    let row: Option<(String, String, i64)> = sqlx::query_as(
        "SELECT j.payload,t.status,length(CAST(j.payload AS BLOB)) \
         FROM review_journal j \
         JOIN threads t ON t.scope_id=j.scope_id AND t.id=j.thread_id \
         WHERE j.scope_id=? AND j.parent_type=? AND j.parent_id=? AND j.discussion_id=? \
         ORDER BY j.version DESC LIMIT 1",
    )
    .bind(ctx.snapshot().scope().as_str())
    .bind(super::discussion_state::discussion_kind_word(
        parent.node_type,
    ))
    .bind(parent.node_id.as_str())
    .bind(discussion_id.as_str())
    .fetch_optional(&mut **tx)
    .await?;
    drop(tx);
    let (payload, status, size) = row.ok_or(ReadFailure::ResourceNotFound)?;
    if size > i64::try_from(RECORD_BYTES)? {
        return Err(ReadFailure::PageRecordTooLarge.into());
    }
    Ok(DiscussionGroup::Addressed {
        discussion: Box::new(serde_json::from_str::<DiscussionEntry>(&payload)?),
        container_status: serde_json::from_value(serde_json::Value::String(status))?,
    })
}
