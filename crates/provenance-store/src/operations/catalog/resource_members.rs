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

projection_member_operation!(GetSourceV2, "get-source-v2", provenance_core::Source);
projection_member_operation!(
    GetResolutionV2,
    "get-resolution-v2",
    provenance_core::Resolution
);
projection_member_operation!(GetRuleV2, "get-rule-v2", provenance_core::Rule);
projection_member_operation!(GetDomainV2, "get-domain-v2", provenance_core::Domain);
projection_member_operation!(GetBoundaryV2, "get-boundary-v2", provenance_core::Boundary);
projection_member_operation!(GetTopicV2, "get-topic-v2", provenance_core::Topic);
projection_member_operation!(GetQuestionV2, "get-question-v2", provenance_core::Question);
projection_member_operation!(
    GetVerificationBindingV2,
    "get-verification-binding-v2",
    provenance_core::VerificationBinding
);
payload_member_operation!(
    GetContributionV2,
    "get-contribution-v2",
    provenance_core::Contribution
);
payload_member_operation!(
    GetSynthesisPacketV2,
    "get-synthesis-packet-v2",
    provenance_core::SynthesisPacket
);
payload_member_operation!(
    GetProposalV2,
    "get-proposal-v2",
    provenance_core::ProposalCard
);
payload_member_operation!(
    GetDiscussionContainerV2,
    "get-discussion-container-v2",
    provenance_core::Thread
);
payload_member_operation!(GetMessageV2, "get-message-v2", provenance_core::Message);
payload_member_operation!(
    GetAssertionV2,
    "get-assertion-v2",
    provenance_core::AssertionRecord
);
payload_member_operation!(
    GetDispositionV2,
    "get-disposition-v2",
    provenance_core::DispositionRecord
);
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
