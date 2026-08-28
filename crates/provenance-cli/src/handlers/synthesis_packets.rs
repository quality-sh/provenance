use super::common::{ideation_target, parse_json_arg, warn_if_skills_missing};
use crate::cli::ideation::SynthesisPacketsCommand;
use crate::output;
use provenance_core::{
    ConsensusFinding, ContestedClaim, EvidenceGap, MinorityObjection, RequiredHumanDecision,
    ScopeId, StableId, SuggestedArtifact, UnsupportedSpeculation,
};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{CreateSynthesisPacketInput, StateStore},
};

pub(super) fn handle(
    command: SynthesisPacketsCommand,
    quiet: bool,
    actor_claim: Option<&provenance_core::RbacClaim>,
) -> anyhow::Result<()> {
    match command {
        SynthesisPacketsCommand::Create {
            repo,
            scope,
            id,
            target_type,
            target_id,
            summary,
            consensus_json,
            contested_claims_json,
            minority_objections_json,
            evidence_gaps_json,
            unsupported_speculation_json,
            open_questions_json,
            suggested_artifacts_json,
            required_human_decisions_json,
            replace,
            format,
        } => {
            warn_if_skills_missing(&repo, quiet)?;
            let store = StateStore::new(ProvenanceLayout::new(repo));
            let input = CreateSynthesisPacketInput {
                scope_id: ScopeId::new(scope)?,
                id: StableId::new(id)?,
                target: ideation_target(&target_type, target_id)?,
                summary,
                consensus: parse_json_arg::<Vec<ConsensusFinding>>(
                    "consensus-json",
                    &consensus_json,
                )?,
                contested_claims: parse_json_arg::<Vec<ContestedClaim>>(
                    "contested-claims-json",
                    &contested_claims_json,
                )?,
                minority_objections: parse_json_arg::<Vec<MinorityObjection>>(
                    "minority-objections-json",
                    &minority_objections_json,
                )?,
                evidence_gaps: parse_json_arg::<Vec<EvidenceGap>>(
                    "evidence-gaps-json",
                    &evidence_gaps_json,
                )?,
                unsupported_speculation: parse_json_arg::<Vec<UnsupportedSpeculation>>(
                    "unsupported-speculation-json",
                    &unsupported_speculation_json,
                )?,
                open_questions: parse_json_arg::<Vec<String>>(
                    "open-questions-json",
                    &open_questions_json,
                )?,
                suggested_artifacts: parse_json_arg::<Vec<SuggestedArtifact>>(
                    "suggested-artifacts-json",
                    &suggested_artifacts_json,
                )?,
                required_human_decisions: parse_json_arg::<Vec<RequiredHumanDecision>>(
                    "required-human-decisions-json",
                    &required_human_decisions_json,
                )?,
            };
            let synthesis_packet = if replace {
                store.upsert_synthesis_packet(actor_claim, input)?
            } else {
                store.create_synthesis_packet(actor_claim, input)?
            };
            output::print(format, &synthesis_packet)?;
        }
        SynthesisPacketsCommand::List {
            repo,
            scope,
            format,
        } => {
            warn_if_skills_missing(&repo, quiet)?;
            let synthesis_packets = StateStore::new(ProvenanceLayout::new(repo))
                .list_synthesis_packets(&ScopeId::new(scope)?)?;
            output::print(format, &synthesis_packets)?;
        }
    }
    Ok(())
}
