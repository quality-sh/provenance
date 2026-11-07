use crate::{cli::ideation::IdeationArtifactKind, output};
use anyhow::Context;
use camino::Utf8Path;
use provenance_core::{
    AssertionRecord, Contribution, DispositionRecord, ProposalCard, SynthesisPacket,
};
use provenance_store::graph_reference::{ExactExport, GraphReference};
use provenance_transport::porcelain::{
    validate_contribution_record, validate_proposal_card_record,
};
use serde::Serialize;

#[derive(Serialize)]
struct ValidationReport {
    artifact: &'static str,
    input: String,
    valid: bool,
}

pub(super) fn handle(artifact: IdeationArtifactKind, input: &Utf8Path) -> anyhow::Result<()> {
    validate_file(artifact, input)?;
    output::print_json(&ValidationReport {
        artifact: artifact.name(),
        input: input.to_string(),
        valid: true,
    })
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
            let _: SynthesisPacket = serde_json::from_str(&json)?;
        }
        IdeationArtifactKind::Proposal => {
            let proposal: ProposalCard = serde_json::from_str(&json)?;
            validate_proposal_card_record(&proposal)?;
        }
        IdeationArtifactKind::Assertion => {
            let assertion: AssertionRecord = serde_json::from_str(&json)?;
            provenance_core::validate_assertion_intrinsic(&assertion)?;
        }
        IdeationArtifactKind::Disposition => {
            let disposition: DispositionRecord = serde_json::from_str(&json)?;
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
    }
    Ok(())
}
