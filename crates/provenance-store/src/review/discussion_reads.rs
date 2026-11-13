use super::discussion_page::{check_stored_size, DiscussionPageFill, Fill};
use crate::operations::{
    read_policy::ReadPolicy,
    reader::{self, Cursor, Position, ReadContext},
};
use crate::read_database::{one_row, SqlColumns, SqlValue};
use crate::repository::Repository;
use provenance_core::{
    model::{ColumnValue, ProjectionRow},
    protocol::{read_failure::ReadFailure, Stamped},
    threads::{Discussion, DiscussionGroup, DiscussionPage, DiscussionQuery},
    StableId, ThreadParent,
};

/// The columns of one Discussion row, in `Discussion::COLUMNS` order.
const DISCUSSION_COLUMNS: &str = "d.scope_id,d.discussion_id,d.parent,\
    d.thread_id,d.root_message_id,d.message_ids,d.status,d.version,d.actor,d.outcomes,\
    d.disposition_id";

/// The stored bytes of one Discussion row that a page accounts for.
pub(super) const DISCUSSION_SIZE: &str = "length(CAST(d.parent AS BLOB))+\
    length(CAST(d.message_ids AS BLOB))+length(CAST(d.outcomes AS BLOB))+\
    length(CAST(d.actor AS BLOB))+256";

/// Selects the Discussions of one parent.
pub(super) const DISCUSSION_PARENT: &str =
    "json_extract(d.parent,'$.node_type')=? AND json_extract(d.parent,'$.node_id')=?";

/// One Discussion from a row of `DISCUSSION_COLUMNS`.
fn discussion_from_row(row: &[SqlValue]) -> anyhow::Result<Discussion> {
    let text = |index| {
        row.text(index)
            .map(|value| ColumnValue::Text(value.to_owned()))
    };
    Discussion::from_row(&[
        text(0)?,
        text(1)?,
        text(2)?,
        text(3)?,
        text(4)?,
        text(5)?,
        text(6)?,
        ColumnValue::Integer(row.integer(7)?),
        text(8)?,
        text(9)?,
        row.optional_text(10)?.map_or(ColumnValue::Null, |value| {
            ColumnValue::Text(value.to_owned())
        }),
    ])
}

pub async fn read_discussions(
    repository: &Repository,
    policy: ReadPolicy,
    query: DiscussionQuery,
) -> anyhow::Result<Stamped<DiscussionPage>> {
    reader::check_limit(query.limit)?;
    let answer =
        reader::answer(repository, policy, move |ctx| Box::pin(groups(ctx, query))).await?;
    crate::operations::queries::page::checked("review-discussions", answer)
}

pub async fn read_discussion(
    repository: &Repository,
    policy: ReadPolicy,
    parent: ThreadParent,
    discussion_id: StableId,
) -> anyhow::Result<Stamped<DiscussionGroup>> {
    reader::answer(repository, policy, move |ctx| {
        Box::pin(group(ctx, parent, discussion_id))
    })
    .await
}

pub(super) async fn check_parent(ctx: &ReadContext, parent: &ThreadParent) -> anyhow::Result<()> {
    let table = super::discussion_state::parent_table(parent.node_type);
    ctx.snapshot().attest(table);
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE scope_id = ? AND id = ?)");
    let rows = ctx
        .snapshot()
        .fetch(
            &sql,
            vec![
                ctx.snapshot().scope_value(),
                SqlValue::from(parent.node_id.as_str()),
            ],
        )
        .await?;
    if one_row(&rows)?.integer(0)? == 0 {
        return Err(ReadFailure::ResourceNotFound.into());
    }
    Ok(())
}

/// The values that select the Discussions of `parent`.
fn parent_values(parent: &ThreadParent) -> [SqlValue; 2] {
    [
        SqlValue::from(super::discussion_state::discussion_kind_word(
            parent.node_type,
        )),
        SqlValue::from(parent.node_id.as_str()),
    ]
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
    for family in ["discussions", "threads", "messages"] {
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

/// Stage 0: adds each addressed discussion after `position`, in discussion
/// id order.
async fn fill_addressed(
    ctx: &ReadContext,
    query: &DiscussionQuery,
    position: &mut Position,
    page: &mut DiscussionPageFill<DiscussionGroup>,
) -> anyhow::Result<Fill> {
    let sql = format!(
        "SELECT d.discussion_id, {DISCUSSION_SIZE}, t.status FROM discussions d \
         JOIN threads t ON t.scope_id=d.scope_id AND t.id=d.thread_id \
         WHERE d.scope_id=? AND {DISCUSSION_PARENT} AND d.discussion_id>? \
         ORDER BY d.discussion_id LIMIT ?"
    );
    let mut values = vec![ctx.snapshot().scope_value()];
    values.extend(parent_values(&query.parent));
    values.push(SqlValue::from(position.id.as_str()));
    values.push(SqlValue::Integer(i64::try_from(query.limit + 1)?));
    let rows = ctx
        .snapshot()
        .fetch(&sql, values)
        .await
        .map_err(reader::page_error)?;
    let mut keys = Vec::with_capacity(rows.len());
    for mut row in rows {
        keys.push((row.take_text(0)?, row.integer(1)?, row.take_text(2)?));
    }
    for (discussion, size, status) in keys {
        if page.at_limit() || page.over_budget(usize::try_from(size)?) {
            return Ok(Fill::Full);
        }
        check_stored_size(size)?;
        let group = discussion_group(ctx, &discussion, status).await?;
        let size = DiscussionPageFill::record_size(&group)?;
        page.push(group, size);
        position.id = discussion;
    }
    Ok(Fill::Complete)
}

/// Loads one Discussion by id as an addressed discussion.
async fn discussion_group(
    ctx: &ReadContext,
    id: &str,
    status: String,
) -> anyhow::Result<DiscussionGroup> {
    let sql = format!(
        "SELECT {DISCUSSION_COLUMNS} FROM discussions d WHERE d.scope_id=? AND d.discussion_id=?"
    );
    let rows = ctx
        .snapshot()
        .fetch(&sql, vec![ctx.snapshot().scope_value(), SqlValue::from(id)])
        .await?;
    addressed(discussion_from_row(one_row(&rows)?)?, status)
}

fn addressed(discussion: Discussion, status: String) -> anyhow::Result<DiscussionGroup> {
    Ok(DiscussionGroup::Addressed {
        discussion: Box::new(discussion),
        container_status: serde_json::from_value(serde_json::Value::String(status))?,
    })
}

/// Stage 1: adds each legacy thread after `position` that holds a message
/// outside every Discussion, in thread id order.
async fn fill_legacy(
    ctx: &ReadContext,
    query: &DiscussionQuery,
    position: &mut Position,
    page: &mut DiscussionPageFill<DiscussionGroup>,
) -> anyhow::Result<Fill> {
    let mut values = vec![ctx.snapshot().scope_value()];
    values.extend(parent_values(&query.parent));
    values.push(SqlValue::from(position.id.as_str()));
    values.push(SqlValue::Integer(i64::try_from(query.limit + 1)?));
    let rows = ctx
        .snapshot()
        .fetch(
            "SELECT t.id,t.status FROM threads t WHERE t.scope_id=? AND t.parent_type=? \
             AND t.parent_id=? AND t.id>? AND EXISTS(SELECT 1 FROM messages m \
             WHERE m.scope_id=t.scope_id AND m.thread_id=t.id AND NOT EXISTS(SELECT 1 \
             FROM discussions d, json_each(d.message_ids) e WHERE d.scope_id=m.scope_id \
             AND e.value=m.id)) ORDER BY t.id LIMIT ?",
            values,
        )
        .await
        .map_err(reader::page_error)?;
    let mut keys = Vec::with_capacity(rows.len());
    for mut row in rows {
        keys.push((row.take_text(0)?, row.take_text(1)?));
    }
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
    for family in ["discussions", "threads"] {
        ctx.snapshot().attest(family);
    }
    let sql = format!(
        "SELECT t.status,{DISCUSSION_SIZE} FROM discussions d \
         JOIN threads t ON t.scope_id=d.scope_id AND t.id=d.thread_id \
         WHERE d.scope_id=? AND {DISCUSSION_PARENT} AND d.discussion_id=?"
    );
    let mut values = vec![ctx.snapshot().scope_value()];
    values.extend(parent_values(&parent));
    values.push(SqlValue::from(discussion_id.as_str()));
    let rows = ctx.snapshot().fetch(&sql, values).await?;
    let row = rows
        .first()
        .map(|row| anyhow::Ok((row.text(0)?.to_owned(), row.integer(1)?)))
        .transpose()?;
    let (status, size) = row.ok_or(ReadFailure::ResourceNotFound)?;
    check_stored_size(size)?;
    discussion_group(ctx, discussion_id.as_str(), status).await
}
