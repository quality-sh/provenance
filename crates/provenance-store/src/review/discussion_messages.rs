use crate::operations::{
    read_policy::ReadPolicy,
    reader::{self, Cursor, Position, ReadContext, PAGE_BYTES, RECORD_BYTES},
};
use camino::Utf8Path;
use provenance_core::{
    protocol::{read_failure::ReadFailure, Stamped},
    threads::{DiscussionMessagesPage, DiscussionMessagesQuery, DiscussionSelector},
    Message, MessageRole, ScopeId, StableId,
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
    let (join, selector_id, condition) = match &selector {
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
    };
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
    if size > i64::try_from(RECORD_BYTES)? {
        return Err(ReadFailure::PageRecordTooLarge.into());
    }
    Ok(Message {
        schema_version: match selector {
            DiscussionSelector::Discussion { .. } => provenance_core::review::REVIEW_SCHEMA_VERSION,
            DiscussionSelector::Legacy { .. } => provenance_core::SUPPORTED_SCHEMA_VERSION,
        },
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
    let (join,selector,condition)=match &query.selector {
        DiscussionSelector::Discussion {discussion_id} => ("JOIN review_journal j ON j.scope_id=m.scope_id AND j.message_id=m.id",discussion_id.as_str(),"j.discussion_id=?"),
        DiscussionSelector::Legacy {thread_id} => ("",thread_id.as_str(),"m.thread_id=? AND NOT EXISTS(SELECT 1 FROM review_journal j WHERE j.scope_id=m.scope_id AND j.message_id=m.id)"),
    };
    if query.cursor.is_none() {
        position.counter = i64::MIN;
    }
    let mut tx = ctx.snapshot().connection().await;
    let kind = super::discussion_state::discussion_kind_word(query.parent.node_type);
    let (address_sql, address_id) = match &query.selector {
        DiscussionSelector::Discussion { discussion_id } => ("SELECT EXISTS(SELECT 1 FROM review_journal WHERE scope_id=? AND parent_type=? AND parent_id=? AND discussion_id=?)", discussion_id.as_str()),
        DiscussionSelector::Legacy { thread_id } => ("SELECT EXISTS(SELECT 1 FROM threads WHERE scope_id=? AND parent_type=? AND parent_id=? AND id=?)", thread_id.as_str()),
    };
    let exists: bool = sqlx::query_scalar(address_sql)
        .bind(ctx.snapshot().scope().as_str())
        .bind(kind)
        .bind(query.parent.node_id.as_str())
        .bind(address_id)
        .fetch_one(&mut **tx)
        .await?;
    if !exists {
        return Err(ReadFailure::ResourceNotFound.into());
    }
    // Fetch lengths and keys first. Large Message bodies never enter the page query.
    let keys: Vec<(String,i64,i64)> = sqlx::query_as(&format!("SELECT m.id,m.created_at,length(CAST(m.body AS BLOB))+COALESCE(length(CAST(m.ai_metadata AS BLOB)),0)+length(m.id)+length(m.thread_id)+256 FROM messages m JOIN threads t ON t.scope_id=m.scope_id AND t.id=m.thread_id {join} WHERE m.scope_id=? AND t.parent_type=? AND t.parent_id=? AND {condition} AND (m.created_at>? OR (m.created_at=? AND m.id>?)) ORDER BY m.created_at,m.id LIMIT ?"))
        .bind(ctx.snapshot().scope().as_str()).bind(kind).bind(query.parent.node_id.as_str()).bind(selector)
        .bind(position.counter).bind(position.counter).bind(&position.id).bind(i64::try_from(query.limit+1)?).fetch_all(&mut **tx).await.map_err(anyhow::Error::from).map_err(reader::page_error)?;
    drop(tx);
    let mut entries = Vec::new();
    let mut bytes = 0;
    for (id, created_at, size) in keys {
        if entries.len() == query.limit {
            return Ok(DiscussionMessagesPage {
                entries,
                next_cursor: Some(cursor.encode(ctx, position)?),
            });
        }
        if size > i64::try_from(RECORD_BYTES)? {
            return Err(ReadFailure::PageRecordTooLarge.into());
        }
        let mut tx = ctx.snapshot().connection().await;
        let (thread, role, body, metadata): (String, String, String, Option<String>) =
            sqlx::query_as(
                "SELECT thread_id,role,body,ai_metadata FROM messages WHERE scope_id=? AND id=?",
            )
            .bind(ctx.snapshot().scope().as_str())
            .bind(&id)
            .fetch_one(&mut **tx)
            .await?;
        drop(tx);
        let message = Message {
            schema_version: match query.selector {
                DiscussionSelector::Discussion { .. } => {
                    provenance_core::review::REVIEW_SCHEMA_VERSION
                }
                DiscussionSelector::Legacy { .. } => provenance_core::SUPPORTED_SCHEMA_VERSION,
            },
            scope_id: ctx.snapshot().scope().clone(),
            id: StableId::new(id.clone())?,
            thread_id: StableId::new(thread)?,
            role: MessageRole::parse(&role)?,
            body,
            created_at,
            ai_metadata: metadata.map(|s| serde_json::from_str(&s)).transpose()?,
        };
        let size = serde_json::to_vec(&message)?.len();
        if size > RECORD_BYTES {
            return Err(ReadFailure::PageRecordTooLarge.into());
        }
        if bytes + size > PAGE_BYTES - 16_384 {
            return Ok(DiscussionMessagesPage {
                entries,
                next_cursor: Some(cursor.encode(ctx, position)?),
            });
        }
        bytes += size + 1;
        entries.push(message);
        position.counter = created_at;
        position.id = id;
    }
    Ok(DiscussionMessagesPage {
        entries,
        next_cursor: None,
    })
}

fn message_cursor(
    ctx: &ReadContext,
    query: &DiscussionMessagesQuery,
    allowed_parent_kinds: Option<&[provenance_core::NodeType]>,
) -> anyhow::Result<(Cursor, Position)> {
    match allowed_parent_kinds {
        Some(kinds) => Cursor::open(
            ctx,
            "discussion-conversation",
            &(&query.parent, &query.selector, query.limit, kinds),
            query.cursor.as_deref(),
        ),
        None => Cursor::open(
            ctx,
            "review-discussion-messages",
            &(&query.parent, &query.selector, query.limit),
            query.cursor.as_deref(),
        ),
    }
}
