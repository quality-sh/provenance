//! Direct typed reads for resource member routes.

use super::review_reads::ReadResult;
use super::{shapes::graph_read_operation, ExecutionNeed};
use crate::cache::read::payloads::{PayloadRow, ProposalPayloadRow};
use crate::operations::reader::{self, ReadContext};
use provenance_core::model::ProjectionRow;
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::StableId;
use serde::Deserialize;

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ResourceMemberRequest {
    pub id: StableId,
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
    (none, $record:ty) => {};
    (
        projection($list:ident, $list_wire:literal, $page:ident, $page_wire:literal, none),
        $record:ty
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
        $record:ty
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
        $record:ty
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
        $record:ty
    ) => {
        projection_member_operation!($member, $wire, $record);
    };
}

macro_rules! define_record_members {
    (
        $(
            $group:ident {
                $(
                    $variant:ident {
                        record: $record:ty,
                        field: $field:ident,
                        path: $path:ident,
                        node: [$($node:tt)*],
                        reader: $reader:ident,
                        closed: [$($closed:tt)*],
                        strategy: $strategy:ident,
                        id: $id:ident,
                        loader: [$($loader:tt)*],
                        catalog: [$($catalog:tt)*]
                    };
                )*
            }
        )*
    ) => {
        $($(catalog_member!($($catalog)*, $record);)*)*
    };
}

crate::cache::record_families!(define_record_members);
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
