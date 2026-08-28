use super::common::{
    complete_flag_set, ideation_target, parse_json_arg, stable_ids, warn_if_skills_missing,
};
use crate::cli::ideation::ProposalsCommand;
use crate::output;
use provenance_core::{
    AssertionId, IdeationEvidenceReference, PromotionState, ProposalTraceability, ProposalType,
    ScopeId, StableId,
};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{CreateAssertionInput, CreateProposalCardInput, ProposalDemand, StateStore},
};

#[allow(clippy::too_many_lines)]
pub(super) fn handle(
    command: ProposalsCommand,
    quiet: bool,
    actor_claim: Option<&provenance_core::RbacClaim>,
) -> anyhow::Result<()> {
    match command {
        ProposalsCommand::Create {
            repo,
            scope,
            id,
            proposal_key,
            proposal_type,
            title,
            summary,
            confidence,
            target_type,
            target_id,
            source_id,
            evidence_json,
            supporting_claim_id,
            assertion_id,
            synthesis_packet_id,
            builds_on,
            format,
        } => {
            warn_if_skills_missing(&repo, quiet)?;
            let store = StateStore::new(ProvenanceLayout::new(repo));
            let scope_id = ScopeId::new(scope)?;
            let proposal_id = StableId::new(id)?;
            let supporting_claim_ids = stable_ids(supporting_claim_id)?;
            let input = CreateProposalCardInput {
                scope_id: scope_id.clone(),
                id: proposal_id.clone(),
                proposal_key,
                proposal_type: ProposalType::parse(&proposal_type)?,
                title,
                summary,
                confidence,
                traceability: ProposalTraceability {
                    target: ideation_target(&target_type, target_id)?,
                    source_ids: stable_ids(source_id)?,
                    evidence_references: parse_json_arg::<Vec<IdeationEvidenceReference>>(
                        "evidence-json",
                        &evidence_json,
                    )?,
                    supporting_claim_ids: supporting_claim_ids.clone(),
                },
                builds_on: builds_on
                    .into_iter()
                    .map(AssertionId::new)
                    .collect::<anyhow::Result<Vec<_>>>()?,
                promotion_state: PromotionState::Proposed,
                duplicate_of: None,
                superseded_by: None,
            };
            let proposal = match (assertion_id, synthesis_packet_id) {
                (Some(assertion_id), Some(synthesis_packet_id)) => store.create_asserted_proposal(
                    actor_claim,
                    input,
                    CreateAssertionInput {
                        scope_id,
                        id: AssertionId::new(assertion_id)?,
                        proposal_id,
                        synthesis_packet_id: StableId::new(synthesis_packet_id)?,
                        supporting_claim_ids,
                    },
                )?,
                (None, None) => store.create_proposal_card(actor_claim, input)?,
                _ => unreachable!("clap requires both atomic assertion arguments"),
            };
            output::print(format, &proposal)?;
        }
        ProposalsCommand::Assert {
            repo,
            scope,
            id,
            proposal_id,
            synthesis_packet_id,
            supporting_claim_id,
            resolve_human_gate,
            decision_key,
            format,
        } => {
            warn_if_skills_missing(&repo, quiet)?;
            let store = StateStore::new(ProvenanceLayout::new(repo));
            let input = CreateAssertionInput {
                scope_id: ScopeId::new(scope)?,
                id: AssertionId::new(id)?,
                proposal_id: StableId::new(proposal_id)?,
                synthesis_packet_id: StableId::new(synthesis_packet_id)?,
                supporting_claim_ids: stable_ids(supporting_claim_id)?,
            };
            let assertion = if resolve_human_gate {
                let decision_keys = stable_ids(decision_key)?;
                store.assert_proposal_after_human_decision(actor_claim, input, &decision_keys)?
            } else {
                store.assert_proposal(actor_claim, input)?
            };
            output::print(format, &assertion)?;
        }
        ProposalsCommand::List {
            repo,
            scope,
            format,
        } => {
            warn_if_skills_missing(&repo, quiet)?;
            let proposals = StateStore::new(ProvenanceLayout::new(repo))
                .list_proposal_cards(&ScopeId::new(scope)?)?;
            output::print(format, &proposals)?;
        }
        ProposalsCommand::Surface {
            repo,
            scope,
            changed_path,
            target_type,
            target_id,
            format,
        } => {
            warn_if_skills_missing(&repo, quiet)?;
            anyhow::ensure!(
                complete_flag_set(
                    &[
                        usize::from(target_type.is_some()),
                        usize::from(target_id.is_some()),
                    ],
                    &[],
                ),
                "--target-type and --target-id must be provided together"
            );
            let targets = target_type
                .zip(target_id)
                .map(|(target_type, target_id)| ideation_target(&target_type, target_id))
                .transpose()?
                .map_or_else(Vec::new, |target| vec![target]);
            let surfaced = StateStore::new(ProvenanceLayout::new(repo)).surface_proposals(
                &ScopeId::new(scope)?,
                &ProposalDemand::new(changed_path, targets),
            )?;
            output::print(format, &surfaced)?;
        }
    }
    Ok(())
}
