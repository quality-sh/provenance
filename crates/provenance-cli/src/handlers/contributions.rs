use super::common::{ideation_target, parse_json_arg, warn_if_skills_missing};
use crate::cli::ideation::ContributionsCommand;
use crate::output;
use provenance_core::{
    ClaimChallenge, ContributionStance, IdeationEvidenceReference, MaterialClaim, ScopeId,
    StableId, SuggestedArtifactChange, UncertaintyLevel, UncertaintyRating,
    UnsupportedRecommendation,
};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{CreateContributionInput, StateStore},
};

pub(super) fn handle(
    command: ContributionsCommand,
    quiet: bool,
    actor_claim: Option<&provenance_core::RbacClaim>,
) -> anyhow::Result<()> {
    match command {
        ContributionsCommand::Create {
            repo,
            scope,
            id,
            target_type,
            target_id,
            participant_slot,
            stance,
            strongest_finding,
            evidence_json,
            claims_json,
            risks_json,
            objections_json,
            challenges_json,
            suggested_changes_json,
            unsupported_recommendations_json,
            uncertainty_level,
            uncertainty_rationale,
            open_questions_json,
            replace,
            format,
        } => {
            warn_if_skills_missing(&repo, quiet)?;
            let store = StateStore::new(ProvenanceLayout::new(repo));
            let input = CreateContributionInput {
                scope_id: ScopeId::new(scope)?,
                id: StableId::new(id)?,
                target: ideation_target(&target_type, target_id)?,
                participant_slot,
                stance: ContributionStance::parse(&stance)?,
                strongest_finding,
                evidence_references: parse_json_arg::<Vec<IdeationEvidenceReference>>(
                    "evidence-json",
                    &evidence_json,
                )?,
                material_claims: parse_json_arg::<Vec<MaterialClaim>>("claims-json", &claims_json)?,
                risks: parse_json_arg::<Vec<String>>("risks-json", &risks_json)?,
                objections: parse_json_arg::<Vec<String>>("objections-json", &objections_json)?,
                challenges: parse_json_arg::<Vec<ClaimChallenge>>(
                    "challenges-json",
                    &challenges_json,
                )?,
                suggested_artifact_changes: parse_json_arg::<Vec<SuggestedArtifactChange>>(
                    "suggested-changes-json",
                    &suggested_changes_json,
                )?,
                unsupported_recommendations: parse_json_arg::<Vec<UnsupportedRecommendation>>(
                    "unsupported-recommendations-json",
                    &unsupported_recommendations_json,
                )?,
                uncertainty: UncertaintyRating {
                    level: UncertaintyLevel::parse(&uncertainty_level)?,
                    rationale: uncertainty_rationale,
                },
                open_questions: parse_json_arg::<Vec<String>>(
                    "open-questions-json",
                    &open_questions_json,
                )?,
            };
            let contribution = if replace {
                store.upsert_contribution(actor_claim, input)?
            } else {
                store.create_contribution(actor_claim, input)?
            };
            output::print(format, &contribution)?;
        }
        ContributionsCommand::List {
            repo,
            scope,
            format,
        } => {
            warn_if_skills_missing(&repo, quiet)?;
            let contributions = StateStore::new(ProvenanceLayout::new(repo))
                .list_contributions(&ScopeId::new(scope)?)?;
            output::print(format, &contributions)?;
        }
    }
    Ok(())
}
