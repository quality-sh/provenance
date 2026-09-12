use super::common::stable_ids;
use super::refs::{self, RequirementList, RequirementSingle};
use crate::cli::knowledge::{FogCommand, RequirementsCommand, SourceRefCommand};
use crate::output;
use provenance_core::{RequirementStatus, ScopeId, StableId};
use provenance_store::{
    layout::ProvenanceLayout,
    operations::catalog,
    state_store::{AddSourceReferenceInput, CreateRequirementInput, StateStore},
};

#[derive(serde::Serialize)]
struct FogView {
    requirement_id: String,
    fog: Option<String>,
}

pub(super) async fn handle(command: RequirementsCommand) -> anyhow::Result<()> {
    match command {
        RequirementsCommand::Update(args) => {
            super::updates::handle::<provenance_store::operations::catalog::UpdateRequirement>(
                args,
            )
            .await?;
        }
        RequirementsCommand::Create {
            repo,
            scope,
            id,
            statement,
            description,
            status,
            domain_id,
            refines,
            spawned_by,
            depends_on,
            supersedes,
            origin_thread,
            origin_message,
            format: _,
        } => {
            let requirement = StateStore::new(ProvenanceLayout::new(repo)).create_requirement(
                CreateRequirementInput {
                    scope_id: ScopeId::new(scope)?,
                    id: StableId::new(id)?,
                    statement,
                    description,
                    status: RequirementStatus::parse(&status)?,
                    domain_id: domain_id.map(StableId::new).transpose()?,
                    refines: refines.map(StableId::new).transpose()?,
                    depends_on: stable_ids(depends_on)?,
                    supersedes: stable_ids(supersedes)?,
                    spawned_by: spawned_by.map(StableId::new).transpose()?,
                    origin_thread: origin_thread.map(StableId::new).transpose()?,
                    origin_message: origin_message.map(StableId::new).transpose()?,
                },
            )?;
            output::print_json(&requirement)?;
        }
        RequirementsCommand::SourceRef { command } => source_ref(command).await?,
        RequirementsCommand::Refines { command } => {
            refs::requirement_single(RequirementSingle::Refines, command).await?;
        }
        RequirementsCommand::DependsOn { command } => {
            refs::requirement_list(RequirementList::DependsOn, command).await?;
        }
        RequirementsCommand::Supersedes { command } => {
            refs::requirement_list(RequirementList::Supersedes, command).await?;
        }
        RequirementsCommand::SpawnedBy { command } => {
            refs::requirement_single(RequirementSingle::SpawnedBy, command).await?;
        }
        RequirementsCommand::Fog { command } => fog(command)?,
    }
    Ok(())
}

async fn source_ref(command: SourceRefCommand) -> anyhow::Result<()> {
    match command {
        SourceRefCommand::Add {
            repo,
            scope,
            requirement_id,
            source_id,
            clause,
            format: _,
        } => {
            let requirement = StateStore::new(ProvenanceLayout::new(repo)).add_source_reference(
                AddSourceReferenceInput {
                    scope_id: ScopeId::new(scope)?,
                    source_id: StableId::new(source_id)?,
                    requirement_id: StableId::new(requirement_id)?,
                    clause,
                },
            )?;
            output::print_json(&requirement)?;
        }
        SourceRefCommand::Clear {
            repo,
            scope,
            requirement_id,
            source_id,
            format: _,
        } => {
            let scope_id = ScopeId::new(&scope)?;
            let requirement = super::native::invoke_native::<catalog::ClearSourceReference>(
                repo,
                scope_id.clone(),
                catalog::ReferenceActionInput {
                    scope_id,
                    id: StableId::new(&requirement_id)?,
                    target_id: StableId::new(&source_id)?,
                },
            )
            .await?;
            output::print_json(&requirement)?;
        }
    }
    Ok(())
}

fn fog(command: FogCommand) -> anyhow::Result<()> {
    match command {
        FogCommand::Set {
            repo,
            scope,
            requirement_id,
            text,
            format: _,
        } => {
            let requirement = StateStore::new(ProvenanceLayout::new(repo)).set_requirement_fog(
                &ScopeId::new(scope)?,
                &StableId::new(requirement_id)?,
                Some(text),
            )?;
            output::print_json(&requirement)?;
        }
        FogCommand::Show {
            repo,
            scope,
            requirement_id,
            format: _,
        } => {
            let requirement_id = StableId::new(requirement_id)?;
            let requirement = StateStore::new(ProvenanceLayout::new(repo))
                .list_requirements(&ScopeId::new(scope)?)?
                .into_iter()
                .find(|requirement| requirement.id == requirement_id)
                .ok_or_else(|| anyhow::anyhow!("requirement does not exist"))?;
            output::print_json(&FogView {
                requirement_id: requirement.id.as_str().to_string(),
                fog: requirement.fog,
            })?;
        }
        FogCommand::Clear {
            repo,
            scope,
            requirement_id,
            format: _,
        } => {
            let requirement = StateStore::new(ProvenanceLayout::new(repo)).set_requirement_fog(
                &ScopeId::new(scope)?,
                &StableId::new(requirement_id)?,
                None,
            )?;
            output::print_json(&requirement)?;
        }
    }
    Ok(())
}
