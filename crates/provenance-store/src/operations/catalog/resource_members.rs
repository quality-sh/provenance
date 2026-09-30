//! Direct typed reads for resource member routes.

use super::review_reads::ReadResult;
use super::{failures::ReadError, shapes::graph_read_operation, ExecutionNeed, PreparedRead};
use crate::cache::read::payloads::{PayloadRow, ProposalPayloadRow};
use crate::operations::reader::{self, ReadContext};
use provenance_core::model::ProjectionRow;
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::review::ReviewRecord;
use provenance_core::StableId;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ResourceMemberRequest {
    pub id: StableId,
}

#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ReviewResource<T> {
    #[serde(flatten)]
    pub record: T,
    pub edit: provenance_core::review::RequirementEditState,
    pub decision: provenance_core::review::RequirementDecisionState,
}

async fn review_resource<T: ProjectionRow>(
    context: &ReadContext,
    scope: provenance_core::ScopeId,
    kind: provenance_core::NodeType,
    id: StableId,
) -> anyhow::Result<ReviewResource<T>> {
    let record = projection_member::<T>(context, ResourceMemberRequest { id: id.clone() }).await?;
    let review_record = ReviewRecord::deserialize_closed(kind, &serde_json::to_value(&record)?)?;
    let store = context
        .live(crate::operations::reader::Live::Canonical)
        .store();
    anyhow::ensure!(review_record.scope_id() == &scope && review_record.id() == &id);
    let snapshot = store.record_review_state(&review_record)?;
    Ok(ReviewResource {
        record,
        edit: snapshot.edit,
        decision: snapshot.decision,
    })
}

async fn reviewed_member<T: ProjectionRow + Send + 'static>(
    read: PreparedRead,
    request: ResourceMemberRequest,
    kind: provenance_core::NodeType,
) -> Result<ReadResult<ReviewResource<T>>, ReadError> {
    let record_scope = read.scope.clone();
    Ok(
        reader::answer(&read.root, &read.scope, read.policy, move |context| {
            Box::pin(review_resource::<T>(
                context,
                record_scope,
                kind,
                request.id,
            ))
        })
        .await?
        .into(),
    )
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ProposalFactMemberRequest {
    pub proposal_id: StableId,
    pub fact_id: StableId,
}

async fn projection_member<K: ProjectionRow>(
    ctx: &ReadContext,
    request: ResourceMemberRequest,
) -> anyhow::Result<K> {
    ctx.snapshot()
        .table::<K>()
        .resource_record(request.id.as_str())
        .await?
        .ok_or_else(|| ReadFailure::ResourceNotFound.into())
}

async fn payload_member<T: PayloadRow>(
    ctx: &ReadContext,
    request: ResourceMemberRequest,
) -> anyhow::Result<T> {
    ctx.snapshot()
        .payloads::<T>()
        .record(request.id.as_str())
        .await?
        .ok_or_else(|| ReadFailure::ResourceNotFound.into())
}

async fn proposal_fact_member<T: ProposalPayloadRow>(
    ctx: &ReadContext,
    request: ProposalFactMemberRequest,
) -> anyhow::Result<T> {
    ctx.snapshot()
        .payloads::<T>()
        .proposal_record(&request.proposal_id, request.fact_id.as_str())
        .await?
        .ok_or_else(|| ReadFailure::ResourceNotFound.into())
}

macro_rules! member_operation {
    ($name:ident, $wire:literal, $request:ty, $result:ty, $read:expr) => {
        graph_read_operation!(
            pub $name,
            $wire,
            $request,
            ReadResult<$result>,
            &[404, 409],
            |_| {
                &[
                    ExecutionNeed::GraphStorage,
                    ExecutionNeed::ProjectionMaintenance,
                ]
            },
            |read, request| async move {
                Ok(
                    reader::answer(&read.root, &read.scope, read.policy, move |ctx| {
                        Box::pin($read(ctx, request))
                    })
                    .await?
                    .into(),
                )
            }
        );
    };
}

macro_rules! projection_member_operation {
    ($name:ident, $wire:literal, $result:ty) => {
        member_operation!(
            $name,
            $wire,
            ResourceMemberRequest,
            $result,
            projection_member::<$result>
        );
    };
}

macro_rules! review_member_operation {
    ($name:ident, $wire:literal, $result:ty, $kind:ident) => {
        graph_read_operation!(
            pub $name,
            $wire,
            ResourceMemberRequest,
            ReadResult<ReviewResource<$result>>,
            &[404, 409],
            |_| {
                &[
                    ExecutionNeed::GraphStorage,
                    ExecutionNeed::ProjectionMaintenance,
                ]
            },
            |read, request| reviewed_member::<$result>(
                read,
                request,
                provenance_core::NodeType::$kind,
            )
        );
    };
}

macro_rules! payload_member_operation {
    ($name:ident, $wire:literal, $result:ty) => {
        member_operation!(
            $name,
            $wire,
            ResourceMemberRequest,
            $result,
            payload_member::<$result>
        );
    };
}

macro_rules! fact_member_operation {
    ($name:ident, $wire:literal, $result:ty) => {
        member_operation!(
            $name,
            $wire,
            ProposalFactMemberRequest,
            $result,
            proposal_fact_member::<$result>
        );
    };
}

macro_rules! catalog_member {
    (none, $record:ty, $($review:tt)*) => {};
    (
        projection($list:ident, $list_wire:literal, $page:ident, $page_wire:literal, none),
        $record:ty,
        $($review:tt)*
    ) => {};
    (
        projection(
            $list:ident,
            $list_wire:literal,
            $page:ident,
            $page_wire:literal,
            $member:ident,
            $wire:literal
        ),
        $record:ty,
        review($kind:ident)
    ) => {
        review_member_operation!($member, $wire, $record, $kind);
    };
    (
        projection(
            $list:ident,
            $list_wire:literal,
            $page:ident,
            $page_wire:literal,
            $member:ident,
            $wire:literal
        ),
        $record:ty,
        none
    ) => {
        projection_member_operation!($member, $wire, $record);
    };
    (
        payload(
            $list:ident,
            $list_wire:literal,
            $page:ident,
            $page_wire:literal,
            $member:ident,
            $wire:literal
        ),
        $record:ty,
        $($review:tt)*
    ) => {
        payload_member_operation!($member, $wire, $record);
    };
    (
        verification(
            $list:ident,
            $list_wire:literal,
            $page:ident,
            $page_wire:literal,
            $member:ident,
            $wire:literal
        ),
        $record:ty,
        $($review:tt)*
    ) => {
        projection_member_operation!($member, $wire, $record);
    };
}

macro_rules! define_record_members {
    (
        $($group:ident {
            $(
                    $variant:ident {
                        record: $record:ty,
                        field: $field:ident,
                        path: $path:ident,
                        meta: $meta:tt,
                        node: [$($node:tt)*],
                        reader: {
                            open: $reader:ident,
                            closed: [$($closed:tt)*],
                            strategy: $strategy:ident
                        },
                        id: $id:ident,
                        loader: [$($loader:tt)*],
                        graph: [$($graph:tt)*],
                        import: [$($import:tt)*],
                        catalog: [$($catalog:tt)*]
                        , route: [$($route:tt)*]
                        $(, review: $review:ident)?
                    };
            )*
        })*
    ) => {
        $($(register_catalog_member!(
            [$($catalog)*], $record, [$($node)*], [$($review)?]
        );)*)*
    };
}

macro_rules! register_catalog_member {
    ([$($catalog:tt)*], $record:ty, [$kind:ident], [$review:ident]) => {
        catalog_member!($($catalog)*, $record, review($kind));
    };
    ([$($catalog:tt)*], $record:ty, [$($kind:tt)*], []) => {
        catalog_member!($($catalog)*, $record, none);
    };
}

crate::cache::family_table::record_family_rows!(define_record_members);
fact_member_operation!(
    GetProposalAssertion,
    "get-proposal-assertion",
    provenance_core::AssertionRecord
);
fact_member_operation!(
    GetProposalDisposition,
    "get-proposal-disposition",
    provenance_core::DispositionRecord
);
