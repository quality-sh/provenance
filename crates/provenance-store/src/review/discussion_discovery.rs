use crate::operations::{
    read_policy::ReadPolicy,
    reader::{self, Cursor, ReadContext, PAGE_BYTES},
};
use camino::Utf8Path;
use provenance_core::{
    protocol::{read_failure::ReadFailure, Stamped},
    threads::{
        DiscussionConversation, DiscussionConversationQuery, DiscussionGroup, DiscussionListPage,
        DiscussionListQuery, DiscussionMessagesQuery, DiscussionSelector, DiscussionStatusFilter,
        DiscussionSummary,
    },
    NodeType, ScopeId, StableId, ThreadParent,
};
use provenance_macros::rule;
use sqlx::{QueryBuilder, Sqlite};

const EXCERPT_CHARS: usize = 240;

/// Lists addressed Discussions in one scope or under one parent.
#[rule("rule_porcelain_discussions_list_scope_or_parent")]
pub async fn read_discussion_list(
    repo: &Utf8Path,
    scope: &ScopeId,
    policy: ReadPolicy,
    query: DiscussionListQuery,
) -> anyhow::Result<Stamped<DiscussionListPage>> {
    super::discussion_reads::check_limit(query.limit)?;
    let answer = reader::answer(repo, scope, policy, move |ctx| Box::pin(list(ctx, query))).await?;
    crate::operations::queries::page::checked("discussion-list", answer)
}

pub async fn read_discussion_conversation(
    repo: &Utf8Path,
    scope: &ScopeId,
    policy: ReadPolicy,
    query: DiscussionConversationQuery,
) -> anyhow::Result<Stamped<DiscussionConversation>> {
    super::discussion_reads::check_limit(query.limit)?;
    let answer = reader::answer(repo, scope, policy, move |ctx| {
        Box::pin(conversation(ctx, query))
    })
    .await?;
    crate::operations::queries::page::checked("discussion-conversation", answer)
}

fn allowed_kinds(kinds: &[NodeType]) -> Vec<&'static str> {
    let mut words = kinds
        .iter()
        .filter_map(|kind| super::discussion_state::discussion_kind_word(*kind))
        .collect::<Vec<_>>();
    words.sort_unstable();
    words.dedup();
    words
}

/// The selected status applies before the page limit.
#[rule("rule_porcelain_discussions_select_status")]
const fn status_word(status: DiscussionStatusFilter) -> Option<&'static str> {
    match status {
        DiscussionStatusFilter::Active => Some("active"),
        DiscussionStatusFilter::Resolved => Some("resolved"),
        DiscussionStatusFilter::All => None,
    }
}

async fn list(ctx: &ReadContext, query: DiscussionListQuery) -> anyhow::Result<DiscussionListPage> {
    let kinds = allowed_kinds(&query.allowed_parent_kinds);
    if let Some(parent) = &query.parent {
        let kind = super::discussion_state::discussion_kind_word(parent.node_type);
        if !kind.is_some_and(|word| kinds.contains(&word)) {
            return Err(ReadFailure::ResourceNotFound.into());
        }
        super::discussion_reads::check_parent(ctx, parent).await?;
    }
    let (cursor, mut position) = Cursor::open(
        ctx,
        "discussion-list",
        &(&query.parent, &kinds, query.status, query.limit),
        query.cursor.as_deref(),
    )?;
    ctx.snapshot().bound_page_work().await?;
    if kinds.is_empty() {
        return Ok(DiscussionListPage {
            entries: Vec::new(),
            next_cursor: None,
        });
    }
    for table in ["review_journal", "messages"] {
        ctx.snapshot().attest(table);
    }
    let mut sql = QueryBuilder::<Sqlite>::new(
        "SELECT j.discussion_id,j.parent_type,j.parent_id,\
         json_extract(j.payload,'$.status'),j.version,\
         substr(m.body,1,241),length(m.body)>240 \
         FROM review_journal j \
         JOIN review_journal first ON first.scope_id=j.scope_id \
           AND first.discussion_id=j.discussion_id AND first.version=1 \
         JOIN messages m ON m.scope_id=first.scope_id AND m.id=first.message_id \
         WHERE j.scope_id=",
    );
    sql.push_bind(ctx.snapshot().scope().as_str());
    sql.push(" AND j.discussion_id>").push_bind(&position.id);
    sql.push(" AND j.parent_type IN (");
    {
        let mut separated = sql.separated(",");
        for kind in &kinds {
            separated.push_bind(*kind);
        }
    }
    sql.push(")");
    if let Some(parent) = &query.parent {
        sql.push(" AND j.parent_type=")
            .push_bind(super::discussion_state::discussion_kind_word(parent.node_type).unwrap());
        sql.push(" AND j.parent_id=")
            .push_bind(parent.node_id.as_str());
    }
    if let Some(status) = status_word(query.status) {
        sql.push(" AND json_extract(j.payload,'$.status')=")
            .push_bind(status);
    }
    sql.push(" AND NOT EXISTS(SELECT 1 FROM review_journal newer WHERE newer.scope_id=j.scope_id AND newer.discussion_id=j.discussion_id AND newer.version>j.version) ORDER BY j.discussion_id LIMIT ")
        .push_bind(i64::try_from(query.limit + 1)?);
    let mut tx = ctx.snapshot().connection().await;
    let rows: Vec<(String, String, String, String, i64, String, i64)> = sql
        .build_query_as()
        .fetch_all(&mut **tx)
        .await
        .map_err(anyhow::Error::from)
        .map_err(reader::page_error)?;
    drop(tx);
    let row_count = rows.len();
    let mut entries = Vec::new();
    let mut bytes = 0;
    for (id, kind, parent_id, status, version, opening, truncated) in rows {
        if entries.len() == query.limit {
            break;
        }
        let entry = summary(
            id.clone(),
            kind,
            parent_id,
            status,
            version,
            opening,
            truncated,
        )?;
        let size = serde_json::to_vec(&entry)?.len();
        if bytes + size > PAGE_BYTES - 16_384 {
            break;
        }
        bytes += size + 1;
        entries.push(entry);
        position.id = id;
    }
    let has_more = row_count > entries.len();
    Ok(DiscussionListPage {
        entries,
        next_cursor: has_more.then(|| cursor.encode(ctx, position)).transpose()?,
    })
}

/// A compact entry gives the stored identity, parent, status, and opening text.
#[rule("rule_porcelain_discussion_list_entry")]
fn summary(
    id: String,
    kind: String,
    parent_id: String,
    status: String,
    version: i64,
    opening: String,
    truncated: i64,
) -> anyhow::Result<DiscussionSummary> {
    let opening_excerpt = opening.chars().take(EXCERPT_CHARS).collect();
    Ok(DiscussionSummary {
        discussion_id: StableId::new(id)?,
        parent: ThreadParent {
            node_type: serde_json::from_value(serde_json::Value::String(kind))?,
            node_id: StableId::new(parent_id)?,
        },
        status: serde_json::from_value(serde_json::Value::String(status))?,
        version: u64::try_from(version)?,
        opening_excerpt,
        excerpt_truncated: truncated != 0,
    })
}

async fn conversation(
    ctx: &ReadContext,
    query: DiscussionConversationQuery,
) -> anyhow::Result<DiscussionConversation> {
    let kinds = allowed_kinds(&query.allowed_parent_kinds);
    if kinds.is_empty() {
        return Err(ReadFailure::ResourceNotFound.into());
    }
    ctx.snapshot().bound_page_work().await?;
    ctx.snapshot().attest("review_journal");
    let mut sql = QueryBuilder::<Sqlite>::new(
        "SELECT parent_type,parent_id FROM review_journal WHERE scope_id=",
    );
    sql.push_bind(ctx.snapshot().scope().as_str());
    sql.push(" AND discussion_id=")
        .push_bind(query.discussion_id.as_str());
    sql.push(" AND parent_type IN (");
    {
        let mut separated = sql.separated(",");
        for kind in &kinds {
            separated.push_bind(*kind);
        }
    }
    sql.push(") ORDER BY version DESC LIMIT 1");
    let mut tx = ctx.snapshot().connection().await;
    let row: Option<(String, String)> = sql.build_query_as().fetch_optional(&mut **tx).await?;
    drop(tx);
    let (kind, id) = row.ok_or(ReadFailure::ResourceNotFound)?;
    let parent = ThreadParent {
        node_type: serde_json::from_value(serde_json::Value::String(kind))?,
        node_id: StableId::new(id)?,
    };
    let DiscussionGroup::Addressed { discussion, .. } =
        super::discussion_reads::group(ctx, parent.clone(), query.discussion_id.clone()).await?
    else {
        unreachable!("an addressed journal row has an addressed group")
    };
    let messages = super::discussion_messages::messages(
        ctx,
        DiscussionMessagesQuery {
            parent,
            selector: DiscussionSelector::Discussion {
                discussion_id: query.discussion_id,
            },
            limit: query.limit,
            cursor: query.cursor,
        },
        Some(query.allowed_parent_kinds),
    )
    .await?;
    Ok(DiscussionConversation {
        head: *discussion,
        messages,
    })
}
