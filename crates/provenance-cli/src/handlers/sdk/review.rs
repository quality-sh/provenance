use crate::{
    cli::sdk::{ScopeArgs, SdkCommand},
    output,
};
use provenance_store::operations::catalog;

pub(super) const fn handles(command: &SdkCommand) -> bool {
    matches!(
        command,
        SdkCommand::RequirementEditState { .. }
            | SdkCommand::SaveRequirement { .. }
            | SdkCommand::RequirementSaveReceipt { .. }
            | SdkCommand::ReviewHistory { .. }
            | SdkCommand::ReviewEvidence { .. }
            | SdkCommand::CreateReviewRequirement { .. }
            | SdkCommand::RequirementCreationReceipt { .. }
            | SdkCommand::WriteDiscussion { .. }
            | SdkCommand::DiscussionReceipt { .. }
            | SdkCommand::ReviewDiscussions { .. }
            | SdkCommand::ReviewDiscussionMessages { .. }
            | SdkCommand::SubmitRequirementReview { .. }
            | SdkCommand::DecideRequirementReview { .. }
            | SdkCommand::WithdrawRequirementReview { .. }
            | SdkCommand::RequirementDecisionState { .. }
            | SdkCommand::RequirementReviewReceipt { .. }
    )
}

async fn print_scope<O>(args: ScopeArgs) -> anyhow::Result<()>
where
    O: catalog::Operation,
    O::Failure: Sync,
{
    output::print_json(&super::submit::<O>(args.repo, args.scope).await?)
}

pub(super) async fn handle(command: SdkCommand) -> anyhow::Result<()> {
    match command {
        SdkCommand::RequirementEditState { scope } => {
            print_scope::<catalog::RequirementEditStateOperation>(scope).await?;
        }
        SdkCommand::SaveRequirement { scope } => {
            print_scope::<catalog::SaveRequirementOperation>(scope).await?;
        }
        SdkCommand::RequirementSaveReceipt { scope } => {
            print_scope::<catalog::RequirementSaveReceiptOperation>(scope).await?;
        }
        SdkCommand::ReviewHistory { query } => {
            output::print_json(
                &super::query::submit::<catalog::ReviewHistoryOperation>(query).await?,
            )?;
        }
        SdkCommand::ReviewEvidence { query } => {
            output::print_json(
                &super::query::submit::<catalog::ReviewEvidenceOperation>(query).await?,
            )?;
        }
        SdkCommand::CreateReviewRequirement { scope } => {
            print_scope::<catalog::CreateReviewRequirementOperation>(scope).await?;
        }
        SdkCommand::RequirementCreationReceipt { scope } => {
            print_scope::<catalog::RequirementCreationReceiptOperation>(scope).await?;
        }
        SdkCommand::WriteDiscussion { scope } => {
            print_scope::<catalog::WriteDiscussionOperation>(scope).await?;
        }
        SdkCommand::DiscussionReceipt { scope } => {
            print_scope::<catalog::DiscussionReceiptOperation>(scope).await?;
        }
        SdkCommand::ReviewDiscussions { query } => output::print_json(
            &super::query::submit::<catalog::ReviewDiscussionsOperation>(query).await?,
        )?,
        SdkCommand::ReviewDiscussionMessages { query } => output::print_json(
            &super::query::submit::<catalog::ReviewDiscussionMessagesOperation>(query).await?,
        )?,
        SdkCommand::SubmitRequirementReview { scope } => {
            print_scope::<catalog::SubmitRequirementReviewOperation>(scope).await?;
        }
        SdkCommand::DecideRequirementReview { scope } => {
            print_scope::<catalog::DecideRequirementReviewOperation>(scope).await?;
        }
        SdkCommand::WithdrawRequirementReview { scope } => {
            print_scope::<catalog::WithdrawRequirementReviewOperation>(scope).await?;
        }
        SdkCommand::RequirementDecisionState { scope } => {
            print_scope::<catalog::RequirementDecisionStateOperation>(scope).await?;
        }
        SdkCommand::RequirementReviewReceipt { scope } => {
            print_scope::<catalog::RequirementReviewReceiptOperation>(scope).await?;
        }
        _ => unreachable!("only review commands reach review dispatch"),
    }
    Ok(())
}
