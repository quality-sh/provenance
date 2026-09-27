//! Bounded typed reads over the live verification-run file.

use super::resource_members::ResourceMemberRequest;
use super::resource_pages::{ResourcePage, VerificationPageRequest};
use super::v2_review_reads::ReadResult;
use super::{shapes::graph_read_operation, ExecutionNeed};
use crate::operations::reader::{self, Cursor, Live, Position, ReadContext, PAGE_BYTES};
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::{ScopeId, VerificationRun};

fn run_page(
    ctx: &ReadContext,
    scope: &ScopeId,
    request: &VerificationPageRequest,
) -> anyhow::Result<ResourcePage<VerificationRun>> {
    anyhow::ensure!(
        (1..=200).contains(&request.limit),
        "resource page limit must be between 1 and 200"
    );
    ctx.live(Live::VerificationRuns)
        .read_runs(scope, |digest, next| {
            let selector = (&request.rule, request.limit);
            let (cursor, mut position) = Cursor::open_live(
                ctx,
                "page-verification-runs-v2",
                &selector,
                request.cursor.as_deref(),
                digest,
            )?;
            let mut items = Vec::new();
            let mut bytes = 0;
            let mut has_more = false;
            while let Some((line, run)) = next()? {
                if line <= position.counter
                    || request
                        .rule
                        .as_ref()
                        .is_some_and(|rule| run.rule_id != *rule)
                {
                    continue;
                }
                let size = serde_json::to_vec(&run)?.len();
                if size > crate::cache::read::page::RESOURCE_RECORD_BYTES {
                    return Err(ReadFailure::PageRecordTooLarge.into());
                }
                if items.len() == request.limit || bytes + size > PAGE_BYTES {
                    has_more = true;
                    break;
                }
                bytes += size;
                position = Position {
                    counter: line,
                    id: run.id.as_str().to_string(),
                    ..Position::default()
                };
                items.push(run);
            }
            Ok(ResourcePage {
                items,
                limit: request.limit,
                has_more,
                next_cursor: has_more.then(|| cursor.encode(ctx, position)).transpose()?,
            })
        })
}

fn run_member(
    ctx: &ReadContext,
    scope: &ScopeId,
    request: &ResourceMemberRequest,
) -> anyhow::Result<VerificationRun> {
    ctx.live(Live::VerificationRuns)
        .read_runs(scope, |_, next| {
            while let Some((_, run)) = next()? {
                if run.id == request.id {
                    return Ok(run);
                }
            }
            Err(ReadFailure::ResourceNotFound.into())
        })
}

graph_read_operation!(
    pub PageVerificationRunsV2,
    "page-verification-runs-v2",
    VerificationPageRequest,
    ReadResult<ResourcePage<VerificationRun>>,
    &[409],
    |_| {
        &[
            ExecutionNeed::GraphStorage,
            ExecutionNeed::ProjectionMaintenance,
            ExecutionNeed::RunStorage,
        ]
    },
    |read, request| async move {
        let scope = read.scope.clone();
        let answer_scope = scope.clone();
        Ok(reader::answer(&read.root, &scope, read.policy, move |ctx| {
            Box::pin(async move { run_page(ctx, &answer_scope, &request) })
        })
        .await?
        .into())
    }
);

graph_read_operation!(
    pub GetVerificationRunV2,
    "get-verification-run-v2",
    ResourceMemberRequest,
    ReadResult<VerificationRun>,
    &[404, 409],
    |_| {
        &[
            ExecutionNeed::GraphStorage,
            ExecutionNeed::ProjectionMaintenance,
            ExecutionNeed::RunStorage,
        ]
    },
    |read, request| async move {
        let scope = read.scope.clone();
        let answer_scope = scope.clone();
        Ok(reader::answer(&read.root, &scope, read.policy, move |ctx| {
            Box::pin(async move { run_member(ctx, &answer_scope, &request) })
        })
        .await?
        .into())
    }
);
