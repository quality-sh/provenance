use crate::{
    cli::ideation::IdeationArtifactKind,
    output::{self, OutputFormat},
};
use anyhow::Context;
use camino::Utf8Path;
// Validating one artifact file asks the same question the ideation aggregate
// asks of a whole scope, so it is the same function: the rule lives in
// `provenance-core` and the message names the record kind.
use provenance_core::{
    ensure_supported_schema_version as ensure_schema_version, validate_optional_confidence_score,
    AssertionRecord, Contribution, DispositionRecord, Manifest, ProposalCard, SynthesisPacket,
};
use provenance_store::graph_reference::{ExactExport, GraphReference};
use serde::Serialize;

#[derive(Serialize)]
struct ValidationReport {
    artifact: &'static str,
    input: String,
    valid: bool,
}

pub(super) fn handle(
    artifact: IdeationArtifactKind,
    input: &Utf8Path,
    format: OutputFormat,
) -> anyhow::Result<()> {
    validate_file(artifact, input)?;
    output::print(
        format,
        &ValidationReport {
            artifact: artifact.name(),
            input: input.to_string(),
            valid: true,
        },
    )
}

pub(super) fn validate_file(
    artifact: IdeationArtifactKind,
    input: &Utf8Path,
) -> anyhow::Result<()> {
    let json = std::fs::read_to_string(input).with_context(|| format!("failed to read {input}"))?;
    match artifact {
        IdeationArtifactKind::Contribution => {
            let contribution: Contribution = serde_json::from_str(&json)?;
            validate_contribution_record(&contribution)?;
        }
        IdeationArtifactKind::SynthesisPacket => {
            let synthesis_packet: SynthesisPacket = serde_json::from_str(&json)?;
            validate_synthesis_packet_record(&synthesis_packet)?;
        }
        IdeationArtifactKind::Proposal => {
            let proposal: ProposalCard = serde_json::from_str(&json)?;
            validate_proposal_card_record(&proposal)?;
        }
        IdeationArtifactKind::Assertion => {
            let assertion: AssertionRecord = serde_json::from_str(&json)?;
            ensure_schema_version("assertion", assertion.schema_version)?;
            provenance_core::validate_assertion_intrinsic(&assertion)?;
        }
        IdeationArtifactKind::Disposition => {
            let disposition: DispositionRecord = serde_json::from_str(&json)?;
            ensure_schema_version("disposition", disposition.schema_version)?;
            provenance_core::validate_disposition_intrinsic(&disposition)?;
        }
        IdeationArtifactKind::GraphReference => {
            GraphReference::from_json(json.as_bytes())?;
        }
        IdeationArtifactKind::GraphReferenceExport => {
            let value = serde_json::from_str(&json)?;
            super::schema::validate_graph_reference_export(&value)?;
            ExactExport::from_json(json.as_bytes())?;
        }
        IdeationArtifactKind::Manifest => {
            let manifest: Manifest = serde_json::from_str(&json)
                .map_err(|error| anyhow::anyhow!("manifest violates its closed schema: {error}"))?;
            provenance_core::ensure_supported_schema_version("manifest", manifest.schema_version)?;
            provenance_core::ensure_unambiguous_rbac(
                &manifest.disposition_actor_ids,
                manifest.rbac.as_ref(),
            )?;
            if let Some(section) = &manifest.rbac {
                provenance_core::ensure_rbac_section_well_formed(section)?;
            }
        }
    }
    Ok(())
}

pub(super) fn validate_contribution_record(contribution: &Contribution) -> anyhow::Result<()> {
    ensure_schema_version("contribution", contribution.schema_version)?;
    for claim in &contribution.material_claims {
        validate_optional_confidence_score(claim.confidence).with_context(|| {
            format!(
                "material claim {} confidence is invalid",
                claim.claim_id.as_str()
            )
        })?;
    }
    Ok(())
}

pub(super) fn validate_synthesis_packet_record(
    synthesis_packet: &SynthesisPacket,
) -> anyhow::Result<()> {
    ensure_schema_version("synthesis", synthesis_packet.schema_version)
}

pub(super) fn validate_proposal_card_record(proposal: &ProposalCard) -> anyhow::Result<()> {
    ensure_schema_version("proposal", proposal.schema_version)?;
    provenance_core::validate_proposal_intrinsic(proposal)?;
    validate_optional_confidence_score(proposal.confidence)
        .with_context(|| format!("proposal {} confidence is invalid", proposal.id.as_str()))?;
    Ok(())
}
