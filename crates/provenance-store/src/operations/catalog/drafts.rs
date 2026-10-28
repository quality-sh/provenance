//! Draft replacements retain the native asserted-evidence and lifecycle gates.
use super::{creation::creation, ExecutionNeed};
use crate::state_store::{
    CreateContributionInput, CreateSynthesisPacketInput, UpdateContributionInput,
    UpdateSynthesisPacketInput,
};
creation!(
    CreateContribution,
    "create-contribution",
    CreateContributionInput,
    provenance_core::Contribution,
    create_contribution,
    [GraphStorage]
);
creation!(
    UpsertContribution,
    "upsert-contribution",
    UpdateContributionInput,
    provenance_core::Contribution,
    update_contribution,
    [GraphStorage]
);
creation!(
    CreateSynthesisPacket,
    "create-synthesis-packet",
    CreateSynthesisPacketInput,
    provenance_core::SynthesisPacket,
    create_synthesis_packet,
    [GraphStorage]
);
creation!(
    UpsertSynthesisPacket,
    "upsert-synthesis-packet",
    UpdateSynthesisPacketInput,
    provenance_core::SynthesisPacket,
    update_synthesis_packet,
    [GraphStorage]
);
