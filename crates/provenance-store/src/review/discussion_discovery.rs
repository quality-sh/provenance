use crate::operations::{
    read_policy::ReadPolicy,
    reader::{self, Cursor, ReadContext, PAGE_BYTES},
};
use crate::read_database::{SqlColumns, SqlRow, SqlValue};
use crate::repository::Repository;
use provenance_core::{
    protocol::{read_failure::ReadFailure, Stamped},
    threads::{
        DiscussionConversation, DiscussionConversationQuery, DiscussionGroup, DiscussionListPage,
        DiscussionListQuery, DiscussionMessagesQuery, DiscussionSelector, DiscussionStatusFilter,
        DiscussionSummary,
    },
    NodeType, StableId, ThreadParent,
};
use provenance_macros::rule;

const EXCERPT_CHARS: usize = 240;
// One Rust char uses at most four UTF-8 bytes.
const EXCERPT_PREFIX_BYTES: usize = EXCERPT_CHARS * 4;

/// Lists addressed Discussions in one scope or under one parent.
#[rule("rule_porcelain_discussions_list_scope_or_parent")]
pub async fn read_discussion_list(
    repository: &Repository,
    policy: ReadPolicy,
    query: DiscussionListQuery,
) -> anyhow::Result<Stamped<DiscussionListPage>> {
    crate::operations::reader::check_limit(query.limit)?;
    let answer = reader::answer(repository, policy, move |ctx| Box::pin(list(ctx, query))).await?;
    crate::operations::queries::page::checked("discussion-list", answer)
}

pub async fn read_discussion_conversation(
    repository: &Repository,
    policy: ReadPolicy,
    query: DiscussionConversationQuery,
) -> anyhow::Result<Stamped<DiscussionConversation>> {
    crate::operations::reader::check_limit(query.limit)?;
    let answer = reader::answer(repository, policy, move |ctx| {
        Box::pin(conversation(ctx, query))
    })
    .await?;
    crate::operations::queries::page::checked("discussion-conversation", answer)
}

fn allowed_kinds(kinds: &[NodeType]) -> Vec<&'static str> {
    let mut words = kinds
        .iter()
        .map(|kind| super::discussion_state::discussion_kind_word(*kind))
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
        if !kinds.contains(&kind) {
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
    let mut tail = String::new();
    let mut tail_values = Vec::new();
    if let Some(parent) = &query.parent {
        tail.push_str(
            " AND json_extract(d.parent,'$.node_type')=? AND json_extract(d.parent,'$.node_id')=?",
        );
        tail_values.push(SqlValue::from(
            super::discussion_state::discussion_kind_word(parent.node_type),
        ));
        tail_values.push(SqlValue::from(parent.node_id.as_str()));
    }
    if let Some(status) = status_word(query.status) {
        tail.push_str(" AND d.status=?");
        tail_values.push(SqlValue::from(status));
    }
    let wanted = query.limit + 1;
    let mut rows = Vec::new();
    for chunk in kinds.chunks(ctx.snapshot().list_values()) {
        let sql = format!(
            "SELECT d.discussion_id,json_extract(d.parent,'$.node_type'),\
             json_extract(d.parent,'$.node_id'),d.status,d.version,\
             substr(CAST(m.body AS BLOB),1,?),length(CAST(m.body AS BLOB)) \
             FROM discussions d \
             JOIN messages m ON m.scope_id=d.scope_id AND m.id=d.root_message_id \
             WHERE d.scope_id=? AND d.discussion_id>? \
             AND json_extract(d.parent,'$.node_type') IN ({}){tail} \
             ORDER BY d.discussion_id LIMIT ?",
            vec!["?"; chunk.len()].join(",")
        );
        let mut values = vec![
            SqlValue::Integer(i64::try_from(EXCERPT_PREFIX_BYTES)?),
            ctx.snapshot().scope_value(),
            SqlValue::from(position.id.as_str()),
        ];
        values.extend(chunk.iter().map(|kind| SqlValue::from(*kind)));
        values.extend(tail_values.iter().cloned());
        values.push(SqlValue::Integer(i64::try_from(wanted)?));
        rows.extend(
            ctx.snapshot()
                .fetch(&sql, values)
                .await
                .map_err(reader::page_error)?,
        );
    }
    let rows = first_by_id(rows, wanted)?;
    let row_count = rows.len();
    let mut entries = Vec::new();
    let mut bytes = 0;
    for mut row in rows {
        if entries.len() == query.limit {
            break;
        }
        let id = row.take_text(0)?;
        let entry = summary(
            id.clone(),
            row.take_text(1)?,
            row.take_text(2)?,
            row.take_text(3)?,
            row.integer(4)?,
            row.blob(5)?,
            row.integer(6)?,
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
    let mut row = None;
    for chunk in kinds.chunks(ctx.snapshot().list_values()) {
        let sql = format!(
            "SELECT json_extract(parent,'$.node_type'),json_extract(parent,'$.node_id') \
             FROM discussions WHERE scope_id=? AND discussion_id=? \
             AND json_extract(parent,'$.node_type') IN ({}) LIMIT 1",
            vec!["?"; chunk.len()].join(",")
        );
        let mut values = vec![
            ctx.snapshot().scope_value(),
            SqlValue::from(query.discussion_id.as_str()),
        ];
        values.extend(chunk.iter().map(|kind| SqlValue::from(*kind)));
        if let Some(mut found) = ctx.snapshot().fetch(&sql, values).await?.into_iter().next() {
            row = Some((found.take_text(0)?, found.take_text(1)?));
            break;
        }
    }
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

/// The first `wanted` rows by discussion id, the first column, of the rows
/// that the statements of each list chunk gave.
fn first_by_id(mut rows: Vec<SqlRow>, wanted: usize) -> anyhow::Result<Vec<SqlRow>> {
    for row in &rows {
        row.text(0)?;
    }
    rows.sort_by(|left, right| {
        left.text(0)
            .unwrap_or_default()
            .cmp(right.text(0).unwrap_or_default())
    });
    rows.truncate(wanted);
    Ok(rows)
}
