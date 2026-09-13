//! Draft replacements retain the native asserted-evidence and lifecycle gates.
use super::{
    creation::creation, ExecutionNeed, ExecutionNeeds, Operation, OperationFuture, PreparedContext,
};
use crate::{
    layout::ProvenanceLayout,
    state_store::{CreateContributionInput, CreateSynthesisPacketInput, StateStore},
    write_error::{SourceFailure, WriteError, WriteFailure},
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
    CreateContributionInput,
    provenance_core::Contribution,
    upsert_contribution,
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
    CreateSynthesisPacketInput,
    provenance_core::SynthesisPacket,
    upsert_synthesis_packet,
    [GraphStorage]
);
