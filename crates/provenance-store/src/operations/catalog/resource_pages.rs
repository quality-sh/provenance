//! Revision-bound pages for resource collection routes.

use super::failures::ReadError;
use super::v2_review_reads::ReadResult;
use super::{shapes::graph_read_operation, ExecutionNeed};
use crate::cache::read::payloads::{PayloadRow, ProposalPayloadRow};
use crate::operations::reader::{self, Cursor, Position, ReadContext, PAGE_BYTES};
use provenance_core::model::ProjectionRow;
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::StableId;
use serde::{Deserialize, Serialize};
use std::future::Future;

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ResourcePageRequest {
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub cursor: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct VerificationPageRequest {
    pub rule: Option<StableId>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub cursor: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProposalFactPageRequest {
    pub proposal_id: StableId,
    #[serde(default = "default_limit")]
    pub limit: usize,
    pub cursor: Option<String>,
}

const fn default_limit() -> usize {
    50
}

#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ResourcePage<T> {
    pub items: Vec<T>,
    pub limit: usize,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

fn validate_limit(limit: usize) -> anyhow::Result<()> {
    anyhow::ensure!(
        (1..=200).contains(&limit),
        "resource page limit must be between 1 and 200"
    );
    Ok(())
}

async fn collect_page<T, F, Fut>(
    ids: Vec<String>,
    limit: usize,
    mut position: Position,
    mut record: F,
) -> anyhow::Result<(Vec<T>, Position, bool)>
where
    T: Serialize,
    F: FnMut(String) -> Fut,
    Fut: Future<Output = anyhow::Result<Option<T>>>,
{
    let mut has_more = ids.len() > limit;
    let mut items = Vec::new();
    let mut bytes = 0;
    for id in ids.into_iter().take(limit) {
        let value = record(id.clone()).await?.ok_or_else(|| {
            anyhow::anyhow!("resource page candidate disappeared inside snapshot")
        })?;
        let size = serde_json::to_vec(&value)?.len();
        if size > crate::cache::read::page::RESOURCE_RECORD_BYTES {
            return Err(ReadFailure::PageRecordTooLarge.into());
        }
        if bytes + size > PAGE_BYTES {
            has_more = true;
            break;
        }
        bytes += size;
        position = Position {
            id,
            ..Position::default()
        };
        items.push(value);
    }
    Ok((items, position, has_more))
}

fn finish<T>(
    ctx: &ReadContext,
    cursor: &Cursor,
    position: Position,
    limit: usize,
    items: Vec<T>,
    has_more: bool,
) -> anyhow::Result<ResourcePage<T>> {
    Ok(ResourcePage {
        items,
        limit,
        has_more,
        next_cursor: has_more.then(|| cursor.encode(ctx, position)).transpose()?,
    })
}

async fn projection_page<K: ProjectionRow>(
    ctx: &ReadContext,
    operation: &str,
    request: ResourcePageRequest,
) -> anyhow::Result<ResourcePage<K>> {
    validate_limit(request.limit)?;
    let (cursor, position) =
        Cursor::open(ctx, operation, &request.limit, request.cursor.as_deref())?;
    ctx.snapshot().bound_page_work().await?;
    let table = ctx.snapshot().table::<K>();
    let ids = table.search_ids(&position.id, request.limit + 1).await?;
    let table = &table;
    let (items, position, has_more) = collect_page(ids, request.limit, position, |id| async move {
        table.resource_record(&id).await
    })
    .await?;
    finish(ctx, &cursor, position, request.limit, items, has_more)
}

async fn payload_page<T: PayloadRow>(
    ctx: &ReadContext,
    operation: &str,
    request: ResourcePageRequest,
) -> anyhow::Result<ResourcePage<T>> {
    validate_limit(request.limit)?;
    let (cursor, position) =
        Cursor::open(ctx, operation, &request.limit, request.cursor.as_deref())?;
    ctx.snapshot().bound_page_work().await?;
    let payloads = ctx.snapshot().payloads::<T>();
    let ids = payloads.ids(&position.id, request.limit + 1).await?;
    let payloads = &payloads;
    let (items, position, has_more) = collect_page(ids, request.limit, position, |id| async move {
        payloads.record(&id).await
    })
    .await?;
    finish(ctx, &cursor, position, request.limit, items, has_more)
}

async fn proposal_fact_page<T: ProposalPayloadRow>(
    ctx: &ReadContext,
    operation: &str,
    request: ProposalFactPageRequest,
) -> anyhow::Result<ResourcePage<T>> {
    validate_limit(request.limit)?;
    let selector = (&request.proposal_id, request.limit);
    let (cursor, position) = Cursor::open(ctx, operation, &selector, request.cursor.as_deref())?;
    ctx.snapshot().bound_page_work().await?;
    let payloads = ctx.snapshot().payloads::<T>();
    let ids = payloads
        .proposal_ids(&request.proposal_id, &position.id, request.limit + 1)
        .await?;
    let proposal = &request.proposal_id;
    let payloads = &payloads;
    let (items, position, has_more) = collect_page(ids, request.limit, position, |id| async move {
        payloads.proposal_record(proposal, &id).await
    })
    .await?;
    finish(ctx, &cursor, position, request.limit, items, has_more)
}

async fn verification_binding_page(
    ctx: &ReadContext,
    request: VerificationPageRequest,
) -> anyhow::Result<ResourcePage<provenance_core::VerificationBinding>> {
    validate_limit(request.limit)?;
    let selector = (&request.rule, request.limit);
    let (cursor, position) = Cursor::open(
        ctx,
        "page-verification-bindings-v2",
        &selector,
        request.cursor.as_deref(),
    )?;
    ctx.snapshot().bound_page_work().await?;
    let table = ctx
        .snapshot()
        .table::<provenance_core::VerificationBinding>();
    let ids = table
        .resource_ids_for_rule(&position.id, request.limit + 1, request.rule.as_ref())
        .await?;
    let table = &table;
    let (items, position, has_more) = collect_page(ids, request.limit, position, |id| async move {
        table.resource_record(&id).await
    })
    .await?;
    finish(ctx, &cursor, position, request.limit, items, has_more)
}

macro_rules! page_operation {
    ($name:ident, $wire:literal, $request:ty, $result:ty, $page:expr) => {
        graph_read_operation!(
            pub $name,
            $wire,
            $request,
            ReadResult<ResourcePage<$result>>,
            &[409],
            |_| {
                &[
                    ExecutionNeed::GraphStorage,
                    ExecutionNeed::ProjectionMaintenance,
                ]
            },
            |read, request| async move {
                Ok(
                    reader::answer(&read.root, &read.scope, read.policy, move |ctx| {
                        Box::pin($page(ctx, $wire, request))
                    })
                    .await?
                    .into(),
                )
            }
        );
    };
}

macro_rules! projection_page_operation {
    ($name:ident, $wire:literal, $result:ty) => {
        page_operation!(
            $name,
            $wire,
            ResourcePageRequest,
            $result,
            projection_page::<$result>
        );
    };
}

macro_rules! payload_page_operation {
    ($name:ident, $wire:literal, $result:ty) => {
        page_operation!(
            $name,
            $wire,
            ResourcePageRequest,
            $result,
            payload_page::<$result>
        );
    };
}

macro_rules! fact_page_operation {
    ($name:ident, $wire:literal, $result:ty) => {
        page_operation!(
            $name,
            $wire,
            ProposalFactPageRequest,
            $result,
            proposal_fact_page::<$result>
        );
    };
}

projection_page_operation!(PageSourcesV2, "page-sources-v2", provenance_core::Source);
projection_page_operation!(
    PageRequirementsV2,
    "page-requirements-v2",
    provenance_core::Requirement
);
projection_page_operation!(
    PageResolutionsV2,
    "page-resolutions-v2",
    provenance_core::Resolution
);
projection_page_operation!(PageRulesV2, "page-rules-v2", provenance_core::Rule);
projection_page_operation!(PageDomainsV2, "page-domains-v2", provenance_core::Domain);
projection_page_operation!(
    PageBoundariesV2,
    "page-boundaries-v2",
    provenance_core::Boundary
);
projection_page_operation!(PageTopicsV2, "page-topics-v2", provenance_core::Topic);
projection_page_operation!(
    PageQuestionsV2,
    "page-questions-v2",
    provenance_core::Question
);
payload_page_operation!(
    PageContributionsV2,
    "page-contributions-v2",
    provenance_core::Contribution
);
payload_page_operation!(
    PageSynthesisPacketsV2,
    "page-synthesis-packets-v2",
    provenance_core::SynthesisPacket
);
payload_page_operation!(
    PageProposalsV2,
    "page-proposals-v2",
    provenance_core::ProposalCard
);
payload_page_operation!(
    PageDiscussionContainersV2,
    "page-discussion-containers-v2",
    provenance_core::Thread
);
payload_page_operation!(PageMessagesV2, "page-messages-v2", provenance_core::Message);
payload_page_operation!(
    PageAssertionsV2,
    "page-assertions-v2",
    provenance_core::AssertionRecord
);
payload_page_operation!(
    PageDispositionsV2,
    "page-dispositions-v2",
    provenance_core::DispositionRecord
);
fact_page_operation!(
    PageProposalAssertionsV2,
    "page-proposal-assertions-v2",
    provenance_core::AssertionRecord
);
fact_page_operation!(
    PageProposalDispositionsV2,
    "page-proposal-dispositions-v2",
    provenance_core::DispositionRecord
);

graph_read_operation!(
    pub PageVerificationBindingsV2,
    "page-verification-bindings-v2",
    VerificationPageRequest,
    ReadResult<ResourcePage<provenance_core::VerificationBinding>>,
    &[409],
    |_| {
        &[
            ExecutionNeed::GraphStorage,
            ExecutionNeed::ProjectionMaintenance,
        ]
    },
    |read, request| async move {
        Ok(
            reader::answer(&read.root, &read.scope, read.policy, move |ctx| {
                Box::pin(verification_binding_page(ctx, request))
            })
            .await?
            .into(),
        )
    }
);
