use super::discussion_page::{check_stored_size, DiscussionPageFill, Fill};
use crate::operations::{
    read_policy::ReadPolicy,
    reader::{self, Cursor, Position, ReadContext},
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
    let mut page = DiscussionPageFill::new(query.limit);
    let mut fill = Fill::Complete;
    if position.stage == 0 {
        fill = fill_addressed(ctx, &query, &mut position, &mut page).await?;
        if fill == Fill::Complete {
            position = Position {
                stage: 1,
                ..Position::default()
            };
        }
    }
    if fill == Fill::Complete {
        fill = fill_legacy(ctx, &query, &mut position, &mut page).await?;
    }
    let next_cursor = match fill {
        Fill::Full => Some(cursor.encode(ctx, position)?),
        Fill::Complete => None,
    };
    Ok(DiscussionPage {
        entries: page.entries,
        next_cursor,
    })
}

/// Stage 0: adds the latest version of each addressed discussion after
/// `position`, in discussion id order.
async fn fill_addressed(
    ctx: &ReadContext,
    query: &DiscussionQuery,
    position: &mut Position,
    page: &mut DiscussionPageFill<DiscussionGroup>,
) -> anyhow::Result<Fill> {
    let mut tx = ctx.snapshot().connection().await;
    let keys: Vec<(String, String, i64, String)> = sqlx::query_as(
        "SELECT j.discussion_id, j.id, length(CAST(j.payload AS BLOB)), t.status FROM review_journal j JOIN threads t ON t.scope_id=j.scope_id AND t.id=j.thread_id WHERE j.scope_id=? AND j.parent_type=? AND j.parent_id=? AND j.discussion_id>? AND NOT EXISTS(SELECT 1 FROM review_journal n WHERE n.scope_id=j.scope_id AND n.discussion_id=j.discussion_id AND n.version>j.version) ORDER BY j.discussion_id LIMIT ?"
    ).bind(ctx.snapshot().scope().as_str())
    .bind(super::discussion_state::discussion_kind_word(query.parent.node_type))
    .bind(query.parent.node_id.as_str()).bind(&position.id)
        .bind(i64::try_from(query.limit + 1)?).fetch_all(&mut **tx).await.map_err(anyhow::Error::from).map_err(reader::page_error)?;
    drop(tx);
    for (discussion, id, size, status) in keys {
        if page.at_limit() || page.over_budget(usize::try_from(size)?) {
            return Ok(Fill::Full);
        }
        check_stored_size(size)?;
        let group = journal_group(ctx, &id, status).await?;
        let size = DiscussionPageFill::record_size(&group)?;
        page.push(group, size);
        position.id = discussion;
    }
    Ok(Fill::Complete)
}

/// Loads one journal entry by id as an addressed discussion.
async fn journal_group(
    ctx: &ReadContext,
    id: &str,
    status: String,
) -> anyhow::Result<DiscussionGroup> {
    let mut tx = ctx.snapshot().connection().await;
    let payload: String =
        sqlx::query_scalar("SELECT payload FROM review_journal WHERE scope_id=? AND id=?")
            .bind(ctx.snapshot().scope().as_str())
            .bind(id)
            .fetch_one(&mut **tx)
            .await?;
    drop(tx);
    addressed(&payload, status)
}

fn addressed(payload: &str, status: String) -> anyhow::Result<DiscussionGroup> {
    Ok(DiscussionGroup::Addressed {
        discussion: Box::new(serde_json::from_str::<DiscussionEntry>(payload)?),
        container_status: serde_json::from_value(serde_json::Value::String(status))?,
    })
}

/// Stage 1: adds each legacy thread after `position` that holds a message
/// outside the review journal, in thread id order.
async fn fill_legacy(
    ctx: &ReadContext,
    query: &DiscussionQuery,
    position: &mut Position,
    page: &mut DiscussionPageFill<DiscussionGroup>,
) -> anyhow::Result<Fill> {
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
        let size = DiscussionPageFill::record_size(&group)?;
        if page.at_limit() || page.over_budget(size) {
            return Ok(Fill::Full);
        }
        page.push(group, size);
        position.id = id;
    }
    Ok(Fill::Complete)
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
    check_stored_size(size)?;
    addressed(&payload, status)
}
