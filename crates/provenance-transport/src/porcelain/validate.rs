//! The validate adapter: one artifact file against its closed schema and its record checks.

use super::swarm_backtrace::{validate_contribution_record, validate_proposal_card_record};
use super::ArtifactPort;
use anyhow::Context;
use camino::Utf8Path;
use provenance_core::{
    AssertionRecord, Contribution, DispositionRecord, ProposalCard, SynthesisPacket,
};
use provenance_porcelain::schema::Artifact;
use provenance_porcelain::validate::{ValidateInput, ValidatePort, ValidationReport};
use provenance_porcelain::WorkFuture;

impl ValidatePort for ArtifactPort {
    type Failure = anyhow::Error;

    fn validate(&self, input: ValidateInput) -> WorkFuture<'_, ValidationReport, anyhow::Error> {
        Box::pin(async move {
            validate_file(input.artifact, Utf8Path::new(&input.input))?;
            Ok(ValidationReport {
                artifact: input.artifact.name().to_owned(),
                input: input.input,
                valid: true,
            })
        })
    }
}

/// Reads one artifact file and runs the checks of its kind.
fn validate_file(artifact: Artifact, input: &Utf8Path) -> anyhow::Result<()> {
    let json = std::fs::read_to_string(input).with_context(|| format!("failed to read {input}"))?;
    match artifact {
        Artifact::Contribution => {
            let contribution: Contribution = serde_json::from_str(&json)?;
            validate_contribution_record(&contribution)?;
        }
        Artifact::SynthesisPacket => {
            let _: SynthesisPacket = serde_json::from_str(&json)?;
        }
        Artifact::Proposal => {
            let proposal: ProposalCard = serde_json::from_str(&json)?;
            validate_proposal_card_record(&proposal)?;
        }
        Artifact::Assertion => {
            let assertion: AssertionRecord = serde_json::from_str(&json)?;
            provenance_core::validate_assertion_intrinsic(&assertion)?;
        }
        Artifact::Disposition => {
            let disposition: DispositionRecord = serde_json::from_str(&json)?;
            provenance_core::validate_disposition_intrinsic(&disposition)?;
        }
    }
    Ok(())
}
