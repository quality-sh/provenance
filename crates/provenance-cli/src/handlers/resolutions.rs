use super::common::{resolution_inputs, stable_ids};
use super::refs::{self, ResolutionList};
use crate::cli::policy::ResolutionsCommand;
use crate::output;
use provenance_core::{ResolutionStatus, ScopeId, StableId};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{CreateResolutionInput, StateStore},
};

pub(super) async fn handle(command: ResolutionsCommand) -> anyhow::Result<()> {
    match command {
        ResolutionsCommand::Update(args) => {
            super::updates::handle::<provenance_store::operations::catalog::UpdateResolution>(args)
                .await?;
        }
        ResolutionsCommand::Create {
            repo,
            scope,
            id,
            title,
            requirement_id,
            supersedes,
            position,
            rationale,
            status,
            context,
            enforcement,
            confidence,
            input_type,
            input_reference,
            input_summary,
            made_by,
            approved_by,
            approved_at,
            origin_thread,
            origin_message,
            format,
        } => {
            let resolution = StateStore::new(ProvenanceLayout::new(repo)).create_resolution(
                CreateResolutionInput {
                    scope_id: ScopeId::new(scope)?,
                    id: StableId::new(id)?,
                    title,
                    requirement_ids: stable_ids(requirement_id)?,
                    supersedes: stable_ids(supersedes)?,
                    position,
                    rationale,
                    status: ResolutionStatus::parse(&status)?,
                    context,
                    enforcement,
                    confidence,
                    inputs: resolution_inputs(input_type, input_reference, input_summary)?,
                    made_by,
                    approved_by,
                    approved_at,
                    origin_thread: origin_thread.map(StableId::new).transpose()?,
                    origin_message: origin_message.map(StableId::new).transpose()?,
                },
            )?;
            output::print(format, &resolution)?;
        }
        ResolutionsCommand::Requirement { command } => {
            refs::resolution_list(ResolutionList::Requirement, command).await?;
        }
        ResolutionsCommand::Supersedes { command } => {
            refs::resolution_list(ResolutionList::Supersedes, command).await?;
        }
    }
    Ok(())
}
