use super::discussion_page::{check_stored_size, DiscussionPageFill, Fill};
use crate::operations::{
    read_policy::ReadPolicy,
    reader::{self, Cursor, Position, ReadContext},
};
use camino::Utf8Path;
use provenance_core::{
    protocol::{read_failure::ReadFailure, Stamped},
    threads::{DiscussionMessagesPage, DiscussionMessagesQuery, DiscussionSelector},
    Message, MessageRole, SchemaVersion, ScopeId, StableId,
};

pub async fn read_discussion_messages(
    repo: &Utf8Path,
    scope: &ScopeId,
    policy: ReadPolicy,
    query: DiscussionMessagesQuery,
) -> anyhow::Result<Stamped<DiscussionMessagesPage>> {
    super::discussion_reads::check_limit(query.limit)?;
    let answer = reader::answer(repo, scope, policy, move |ctx| {
        Box::pin(messages(ctx, query, None))
    })
    .await?;
    crate::operations::queries::page::checked("review-discussion-messages", answer)
}

pub async fn read_discussion_message(
    repo: &Utf8Path,
    scope: &ScopeId,
    policy: ReadPolicy,
    parent: provenance_core::ThreadParent,
    selector: DiscussionSelector,
    message_id: StableId,
) -> anyhow::Result<Stamped<Message>> {
    reader::answer(repo, scope, policy, move |ctx| {
        Box::pin(message(ctx, parent, selector, message_id))
    })
    .await
}

async fn message(
    ctx: &ReadContext,
    parent: provenance_core::ThreadParent,
    selector: DiscussionSelector,
    message_id: StableId,
) -> anyhow::Result<Message> {
    ctx.snapshot().bound_page_work().await?;
    super::discussion_reads::check_parent(ctx, &parent).await?;
    for family in ["review_journal", "messages", "threads"] {
        ctx.snapshot().attest(family);
    }
    let (join, selector_id, condition) = selector_sql(&selector);
    let kind = super::discussion_state::discussion_kind_word(parent.node_type);
    let sql = format!(
        "SELECT m.thread_id,m.role,m.body,m.created_at,m.ai_metadata,\
         length(CAST(m.body AS BLOB))+COALESCE(length(CAST(m.ai_metadata AS BLOB)),0)+\
         length(m.id)+length(m.thread_id)+256 FROM messages m \
         JOIN threads t ON t.scope_id=m.scope_id AND t.id=m.thread_id {join} \
         WHERE m.scope_id=? AND t.parent_type=? AND t.parent_id=? AND {condition} AND m.id=?"
    );
    let mut tx = ctx.snapshot().connection().await;
    let row: Option<(String, String, String, i64, Option<String>, i64)> = sqlx::query_as(&sql)
        .bind(ctx.snapshot().scope().as_str())
        .bind(kind)
        .bind(parent.node_id.as_str())
        .bind(selector_id)
        .bind(message_id.as_str())
        .fetch_optional(&mut **tx)
        .await?;
    drop(tx);
    let (thread, role, body, created_at, metadata, size) =
        row.ok_or(ReadFailure::ResourceNotFound)?;
    check_stored_size(size)?;
    Ok(Message {
        schema_version: message_schema_version(&selector),
        scope_id: ctx.snapshot().scope().clone(),
        id: message_id,
        thread_id: StableId::new(thread)?,
        role: MessageRole::parse(&role)?,
        body,
        created_at,
        ai_metadata: metadata
            .map(|value| serde_json::from_str(&value))
            .transpose()?,
    })
}

pub(super) async fn messages(
    ctx: &ReadContext,
    query: DiscussionMessagesQuery,
    allowed_parent_kinds: Option<&[provenance_core::NodeType]>,
) -> anyhow::Result<DiscussionMessagesPage> {
    let (cursor, mut position) = message_cursor(ctx, &query, allowed_parent_kinds)?;
    ctx.snapshot().bound_page_work().await?;
    super::discussion_reads::check_parent(ctx, &query.parent).await?;
    for family in ["review_journal", "messages", "threads"] {
        ctx.snapshot().attest(family);
    }
    if query.cursor.is_none() {
        position.counter = i64::MIN;
    }
    check_address(ctx, &query).await?;
    let mut page = DiscussionPageFill::new(query.limit);
    let next_cursor = match fill_messages(ctx, &query, &mut position, &mut page).await? {
        Fill::Full => Some(cursor.encode(ctx, position)?),
        Fill::Complete => None,
    };
    Ok(DiscussionMessagesPage {
        entries: page.entries,
        next_cursor,
    })
}

/// The join, the selected id, and the condition that select the messages
/// of one discussion or of one legacy thread.
fn selector_sql(selector: &DiscussionSelector) -> (&'static str, &str, &'static str) {
    match selector {
        DiscussionSelector::Discussion { discussion_id } => (
            "JOIN review_journal j ON j.scope_id=m.scope_id AND j.message_id=m.id",
            discussion_id.as_str(),
            "j.discussion_id=?",
        ),
        DiscussionSelector::Legacy { thread_id } => (
            "",
            thread_id.as_str(),
            "m.thread_id=? AND NOT EXISTS(SELECT 1 FROM review_journal j WHERE j.scope_id=m.scope_id AND j.message_id=m.id)",
        ),
    }
}

const fn message_schema_version(selector: &DiscussionSelector) -> SchemaVersion {
    match selector {
        DiscussionSelector::Discussion { .. } => provenance_core::review::REVIEW_SCHEMA_VERSION,
        DiscussionSelector::Legacy { .. } => provenance_core::SUPPORTED_SCHEMA_VERSION,
    }
}

/// Refuses a discussion or a legacy thread that the parent does not hold.
async fn check_address(ctx: &ReadContext, query: &DiscussionMessagesQuery) -> anyhow::Result<()> {
    let (address_sql, address_id) = match &query.selector {
        DiscussionSelector::Discussion { discussion_id } => ("SELECT EXISTS(SELECT 1 FROM review_journal WHERE scope_id=? AND parent_type=? AND parent_id=? AND discussion_id=?)", discussion_id.as_str()),
        DiscussionSelector::Legacy { thread_id } => ("SELECT EXISTS(SELECT 1 FROM threads WHERE scope_id=? AND parent_type=? AND parent_id=? AND id=?)", thread_id.as_str()),
    };
    let kind = super::discussion_state::discussion_kind_word(query.parent.node_type);
    let mut tx = ctx.snapshot().connection().await;
    let exists: bool = sqlx::query_scalar(address_sql)
        .bind(ctx.snapshot().scope().as_str())
        .bind(kind)
        .bind(query.parent.node_id.as_str())
        .bind(address_id)
        .fetch_one(&mut **tx)
        .await?;
    drop(tx);
    if !exists {
        return Err(ReadFailure::ResourceNotFound.into());
    }
    Ok(())
}

/// Adds the messages after `position` to the page, in creation order, and
/// moves `position` past each message it adds.
async fn fill_messages(
    ctx: &ReadContext,
    query: &DiscussionMessagesQuery,
    position: &mut Position,
    page: &mut DiscussionPageFill<Message>,
) -> anyhow::Result<Fill> {
    for (id, created_at, size) in message_keys(ctx, query, position).await? {
        if page.at_limit() {
            return Ok(Fill::Full);
        }
        check_stored_size(size)?;
        let message = page_message(ctx, &query.selector, &id, created_at).await?;
        let size = DiscussionPageFill::record_size(&message)?;
        if page.over_budget(size) {
            return Ok(Fill::Full);
        }
        page.push(message, size);
        position.counter = created_at;
        position.id = id;
    }
    Ok(Fill::Complete)
}

/// The id, creation time, and stored size of each message after
/// `position`. Large Message bodies never enter this query.
async fn message_keys(
    ctx: &ReadContext,
    query: &DiscussionMessagesQuery,
    position: &Position,
) -> anyhow::Result<Vec<(String, i64, i64)>> {
    let (join, selector, condition) = selector_sql(&query.selector);
    let kind = super::discussion_state::discussion_kind_word(query.parent.node_type);
    let mut tx = ctx.snapshot().connection().await;
    let keys: Vec<(String, i64, i64)> = sqlx::query_as(&format!("SELECT m.id,m.created_at,length(CAST(m.body AS BLOB))+COALESCE(length(CAST(m.ai_metadata AS BLOB)),0)+length(m.id)+length(m.thread_id)+256 FROM messages m JOIN threads t ON t.scope_id=m.scope_id AND t.id=m.thread_id {join} WHERE m.scope_id=? AND t.parent_type=? AND t.parent_id=? AND {condition} AND (m.created_at>? OR (m.created_at=? AND m.id>?)) ORDER BY m.created_at,m.id LIMIT ?"))
        .bind(ctx.snapshot().scope().as_str()).bind(kind).bind(query.parent.node_id.as_str()).bind(selector)
        .bind(position.counter).bind(position.counter).bind(&position.id).bind(i64::try_from(query.limit+1)?).fetch_all(&mut **tx).await.map_err(anyhow::Error::from).map_err(reader::page_error)?;
    drop(tx);
    Ok(keys)
}

/// Loads one message of the page by id.
async fn page_message(
    ctx: &ReadContext,
    selector: &DiscussionSelector,
    id: &str,
    created_at: i64,
) -> anyhow::Result<Message> {
    let mut tx = ctx.snapshot().connection().await;
    let (thread, role, body, metadata): (String, String, String, Option<String>) = sqlx::query_as(
        "SELECT thread_id,role,body,ai_metadata FROM messages WHERE scope_id=? AND id=?",
    )
    .bind(ctx.snapshot().scope().as_str())
    .bind(id)
    .fetch_one(&mut **tx)
    .await?;
    drop(tx);
    Ok(Message {
        schema_version: message_schema_version(selector),
        scope_id: ctx.snapshot().scope().clone(),
        id: StableId::new(id)?,
        thread_id: StableId::new(thread)?,
        role: MessageRole::parse(&role)?,
        body,
        created_at,
        ai_metadata: metadata.map(|s| serde_json::from_str(&s)).transpose()?,
    })
}

fn message_cursor(
    ctx: &ReadContext,
    query: &DiscussionMessagesQuery,
    allowed_parent_kinds: Option<&[provenance_core::NodeType]>,
) -> anyhow::Result<(Cursor, Position)> {
    allowed_parent_kinds.map_or_else(
        || {
            Cursor::open(
                ctx,
                "review-discussion-messages",
                &(&query.parent, &query.selector, query.limit),
                query.cursor.as_deref(),
            )
        },
        |kinds| {
            Cursor::open(
                ctx,
                "discussion-conversation",
                &(&query.parent, &query.selector, query.limit, kinds),
                query.cursor.as_deref(),
            )
        },
    )
}
