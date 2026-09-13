use super::common::{ideation_target, parse_json_arg, warn_if_skills_missing};
use crate::cli::ideation::ContributionsCommand;
use crate::output;
use crate::store::Store;
use provenance_core::{
    ClaimChallenge, ContributionStance, IdeationEvidenceReference, MaterialClaim, ScopeId,
    StableId, SuggestedArtifactChange, UncertaintyLevel, UncertaintyRating,
    UnsupportedRecommendation,
};
use provenance_store::state_store::CreateContributionInput;

pub(super) fn handle(command: ContributionsCommand, quiet: bool) -> anyhow::Result<()> {
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
            ..
        } => {
            warn_if_skills_missing(&repo, quiet)?;
            let store = Store::open(repo);
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
                store.upsert_contribution(input)?
            } else {
                store.create_contribution(input)?
            };
            output::print_json(&contribution)?;
        }
        ContributionsCommand::List { repo, scope, .. } => {
            warn_if_skills_missing(&repo, quiet)?;
            let contributions = Store::open(repo).list_contributions(&ScopeId::new(scope)?)?;
            output::print_json(&contributions)?;
        }
    }
    Ok(())
}
