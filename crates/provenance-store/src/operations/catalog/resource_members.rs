//! Direct typed reads for resource member routes.

use super::v2_review_reads::ReadResult;
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
    (projection($list:ident, $list_wire:literal, $page:ident, $page_wire:literal, none), $record:ty) => {};
    (projection($list:ident, $list_wire:literal, $page:ident, $page_wire:literal, $member:ident, $wire:literal), $record:ty) => {
        projection_member_operation!($member, $wire, $record);
    };
    (payload($list:ident, $list_wire:literal, $page:ident, $page_wire:literal, $member:ident, $wire:literal), $record:ty) => {
        payload_member_operation!($member, $wire, $record);
    };
    (verification($list:ident, $list_wire:literal, $page:ident, $member:ident, $wire:literal), $record:ty) => {
        projection_member_operation!($member, $wire, $record);
    };
}

macro_rules! define_record_members {
    (
        export { $($export_variant:ident: $export_type:ty, $export_field:ident, $export_path:ident, $export_suffix:literal, $export_table:literal, [$($export_node:tt)*], $export_reader:ident, [$($export_closed:tt)*], $export_id:ident, [$($export_loader:tt)*], [$($export_catalog:tt)*];)* }
        canonical { $($canonical_variant:ident: $canonical_type:ty, $canonical_field:ident, $canonical_path:ident, $canonical_suffix:literal, $canonical_table:literal, [$($canonical_node:tt)*], $canonical_reader:ident, [$($canonical_closed:tt)*], $canonical_id:ident, [$($canonical_loader:tt)*], [$($canonical_catalog:tt)*];)* }
        internal { $($internal_variant:ident: $internal_type:ty, $internal_field:ident, $internal_path:ident, $internal_suffix:literal, $internal_table:literal, [$($internal_node:tt)*], $internal_reader:ident, [$($internal_closed:tt)*], $internal_id:ident, [$($internal_loader:tt)*], [$($internal_catalog:tt)*];)* }
    ) => {
        $(catalog_member!($($export_catalog)*, $export_type);)*
        $(catalog_member!($($canonical_catalog)*, $canonical_type);)*
        $(catalog_member!($($internal_catalog)*, $internal_type);)*
    };
}

crate::cache::record_families!(define_record_members);
fact_member_operation!(
    GetProposalAssertionV2,
    "get-proposal-assertion-v2",
    provenance_core::AssertionRecord
);
fact_member_operation!(
    GetProposalDispositionV2,
    "get-proposal-disposition-v2",
    provenance_core::DispositionRecord
);
