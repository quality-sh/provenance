use crate::operations::{
    read_policy::ReadPolicy,
    reader::{self, Cursor, ReadContext, PAGE_BYTES, RECORD_BYTES},
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
        Box::pin(messages(ctx, query))
    })
    .await?;
    crate::operations::queries::page::checked("review-discussion-messages", answer)
}

async fn messages(
    ctx: &ReadContext,
    query: DiscussionMessagesQuery,
) -> anyhow::Result<DiscussionMessagesPage> {
    let (cursor, mut position) = Cursor::open(
        ctx,
        "review-discussion-messages",
        &(&query.parent, &query.selector, query.limit),
        query.cursor.as_deref(),
    )?;
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
    let (address_sql, address_id) = match &query.selector {
        DiscussionSelector::Discussion { discussion_id } => ("SELECT EXISTS(SELECT 1 FROM review_journal WHERE scope_id=? AND parent_type='requirement' AND parent_id=? AND discussion_id=?)", discussion_id),
        DiscussionSelector::Legacy { thread_id } => ("SELECT EXISTS(SELECT 1 FROM threads WHERE scope_id=? AND parent_type='requirement' AND parent_id=? AND id=?)", thread_id),
    };
    let exists: bool = sqlx::query_scalar(address_sql)
        .bind(ctx.snapshot().scope().as_str())
        .bind(query.parent.node_id.as_str())
        .bind(address_id.as_str())
        .fetch_one(&mut **tx)
        .await?;
    anyhow::ensure!(
        exists,
        "Discussion selector does not belong to this parent and scope"
    );
    // Fetch lengths and keys first. Large Message bodies never enter the page query.
    let keys: Vec<(String,i64,i64)> = sqlx::query_as(&format!("SELECT m.id,m.created_at,length(CAST(m.body AS BLOB))+COALESCE(length(CAST(m.ai_metadata AS BLOB)),0)+length(m.id)+length(m.thread_id)+256 FROM messages m JOIN threads t ON t.scope_id=m.scope_id AND t.id=m.thread_id {join} WHERE m.scope_id=? AND t.parent_type='requirement' AND t.parent_id=? AND {condition} AND (m.created_at>? OR (m.created_at=? AND m.id>?)) ORDER BY m.created_at,m.id LIMIT ?"))
        .bind(ctx.snapshot().scope().as_str()).bind(query.parent.node_id.as_str()).bind(selector)
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
