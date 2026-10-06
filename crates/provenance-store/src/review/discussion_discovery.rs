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
// One Rust char uses at most four UTF-8 bytes.
const EXCERPT_PREFIX_BYTES: usize = EXCERPT_CHARS * 4;
type DiscussionListRow = (String, String, String, String, i64, Vec<u8>, i64);

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
    for table in ["discussions", "messages"] {
        ctx.snapshot().attest(table);
    }
    let mut sql = QueryBuilder::<Sqlite>::new(
        "SELECT d.discussion_id,json_extract(d.parent,'$.node_type'),\
         json_extract(d.parent,'$.node_id'),d.status,d.version,\
         substr(CAST(m.body AS BLOB),1,",
    );
    sql.push_bind(i64::try_from(EXCERPT_PREFIX_BYTES)?);
    sql.push(
        "),length(CAST(m.body AS BLOB)) \
         FROM discussions d \
         JOIN messages m ON m.scope_id=d.scope_id AND m.id=d.root_message_id \
         WHERE d.scope_id=",
    );
    sql.push_bind(ctx.snapshot().scope().as_str());
    sql.push(" AND d.discussion_id>").push_bind(&position.id);
    sql.push(" AND json_extract(d.parent,'$.node_type') IN (");
    {
        let mut separated = sql.separated(",");
        for kind in &kinds {
            separated.push_bind(*kind);
        }
    }
    sql.push(")");
    if let Some(parent) = &query.parent {
        sql.push(" AND json_extract(d.parent,'$.node_type')=")
            .push_bind(super::discussion_state::discussion_kind_word(parent.node_type).unwrap());
        sql.push(" AND json_extract(d.parent,'$.node_id')=")
            .push_bind(parent.node_id.as_str());
    }
    if let Some(status) = status_word(query.status) {
        sql.push(" AND d.status=").push_bind(status);
    }
    sql.push(" ORDER BY d.discussion_id LIMIT ")
        .push_bind(i64::try_from(query.limit + 1)?);
    let mut tx = ctx.snapshot().connection().await;
    let rows: Vec<DiscussionListRow> = sql
        .build_query_as()
        .fetch_all(&mut **tx)
        .await
        .map_err(anyhow::Error::from)
        .map_err(reader::page_error)?;
    drop(tx);
    let row_count = rows.len();
    let mut entries = Vec::new();
    let mut bytes = 0;
    for (id, kind, parent_id, status, version, opening, body_bytes) in rows {
        if entries.len() == query.limit {
            break;
        }
        let entry = summary(
            id.clone(),
            kind,
            parent_id,
            status,
            version,
            &opening,
            body_bytes,
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
    opening: &[u8],
    body_bytes: i64,
) -> anyhow::Result<DiscussionSummary> {
    let prefix = match std::str::from_utf8(opening) {
        Ok(prefix) => prefix,
        Err(error) if error.error_len().is_none() => {
            std::str::from_utf8(&opening[..error.valid_up_to()])?
        }
        Err(error) => return Err(error.into()),
    };
    let opening_excerpt = prefix.chars().take(EXCERPT_CHARS).collect();
    let excerpt_truncated =
        usize::try_from(body_bytes)? > opening.len() || prefix.chars().nth(EXCERPT_CHARS).is_some();
    Ok(DiscussionSummary {
        discussion_id: StableId::new(id)?,
        parent: ThreadParent {
            node_type: serde_json::from_value(serde_json::Value::String(kind))?,
            node_id: StableId::new(parent_id)?,
        },
        status: serde_json::from_value(serde_json::Value::String(status))?,
        version: u64::try_from(version)?,
        opening_excerpt,
        excerpt_truncated,
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
    ctx.snapshot().attest("discussions");
    let mut sql = QueryBuilder::<Sqlite>::new(
        "SELECT json_extract(parent,'$.node_type'),json_extract(parent,'$.node_id') \
         FROM discussions WHERE scope_id=",
    );
    sql.push_bind(ctx.snapshot().scope().as_str());
    sql.push(" AND discussion_id=")
        .push_bind(query.discussion_id.as_str());
    sql.push(" AND json_extract(parent,'$.node_type') IN (");
    {
        let mut separated = sql.separated(",");
        for kind in &kinds {
            separated.push_bind(*kind);
        }
    }
    sql.push(") LIMIT 1");
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
        unreachable!("a Discussion row has an addressed group")
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
        Some(&query.allowed_parent_kinds),
    )
    .await?;
    Ok(DiscussionConversation {
        head: *discussion,
        messages,
    })
}
