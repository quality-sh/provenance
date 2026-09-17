//! Revision-bound pages for resource collection routes.

use super::failures::ReadError;
use super::v2_review_reads::ReadResult;
use super::{
    ContextKind, ExecutionNeed, ExecutionNeeds, Operation, OperationFuture, PreparedContext,
};
use crate::operations::reader::{self, Cursor, Position, ReadContext, PAGE_BYTES, RECORD_BYTES};
use provenance_core::model::ProjectionRow;
use provenance_core::protocol::read_failure::ReadFailure;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ResourcePageRequest {
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

async fn page<K: ProjectionRow>(
    ctx: &ReadContext,
    operation: &str,
    request: ResourcePageRequest,
) -> anyhow::Result<ResourcePage<K>> {
    anyhow::ensure!(
        (1..=200).contains(&request.limit),
        "resource page limit must be between 1 and 200"
    );
    let (cursor, mut position) =
        Cursor::open(ctx, operation, &request.limit, request.cursor.as_deref())?;
    ctx.snapshot().bound_page_work().await?;
    let table = ctx.snapshot().table::<K>();
    let ids = table.search_ids(&position.id, request.limit + 1).await?;
    let mut has_more = ids.len() > request.limit;
    let mut items = Vec::new();
    let mut bytes = 0;
    for id in ids.into_iter().take(request.limit) {
        let record = table.page_record(&id).await?.ok_or_else(|| {
            anyhow::anyhow!("resource page candidate disappeared inside snapshot")
        })?;
        let size = serde_json::to_vec(&record)?.len();
        if size > RECORD_BYTES {
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
        items.push(record);
    }
    Ok(ResourcePage {
        items,
        limit: request.limit,
        has_more,
        next_cursor: has_more.then(|| cursor.encode(ctx, position)).transpose()?,
    })
}

macro_rules! resource_page {
    ($name:ident, $wire:literal, $result:ty) => {
        pub struct $name;
        impl Operation for $name {
            type Request = ResourcePageRequest;
            type Success = ReadResult<ResourcePage<$result>>;
            type Failure = ReadError;
            const NAME: &'static str = $wire;
            const CONTEXT: ContextKind = ContextKind::Scoped;
            const FAILURE_STATUSES: &'static [u16] = &[409];
            fn needs(_: &Self::Request) -> ExecutionNeeds {
                &[
                    ExecutionNeed::GraphStorage,
                    ExecutionNeed::ProjectionMaintenance,
                ]
            }
            fn failure_status(error: &ReadError) -> u16 {
                error.status()
            }
            fn run(
                context: PreparedContext,
                request: Self::Request,
            ) -> OperationFuture<Self::Success, Self::Failure> {
                Box::pin(async move {
                    let read = context.graph()?;
                    Ok(
                        reader::answer(&read.root, &read.scope, read.policy, move |ctx| {
                            Box::pin(page::<$result>(ctx, $wire, request))
                        })
                        .await?
                        .into(),
                    )
                })
            }
        }
    };
}

resource_page!(PageSourcesV2, "page-sources-v2", provenance_core::Source);
resource_page!(
    PageRequirementsV2,
    "page-requirements-v2",
    provenance_core::Requirement
);
resource_page!(
    PageResolutionsV2,
    "page-resolutions-v2",
    provenance_core::Resolution
);
resource_page!(PageRulesV2, "page-rules-v2", provenance_core::Rule);
resource_page!(PageDomainsV2, "page-domains-v2", provenance_core::Domain);
resource_page!(
    PageBoundariesV2,
    "page-boundaries-v2",
    provenance_core::Boundary
);
resource_page!(PageTopicsV2, "page-topics-v2", provenance_core::Topic);
resource_page!(
    PageQuestionsV2,
    "page-questions-v2",
    provenance_core::Question
);
