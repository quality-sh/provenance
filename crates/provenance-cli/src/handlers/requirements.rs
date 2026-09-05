use super::common::stable_ids;
use super::references::{self, RequirementList, RequirementSingle};
use crate::cli::knowledge::{FogCommand, RequirementsCommand, SourceRefCommand};
use crate::output;
use provenance_core::{RequirementStatus, ScopeId, StableId};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{AddSourceReferenceInput, CreateRequirementInput, StateStore},
};

#[derive(serde::Serialize)]
struct FogView {
    requirement_id: String,
    fog: Option<String>,
}

pub(super) fn handle(command: RequirementsCommand) -> anyhow::Result<()> {
    match command {
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
            format,
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
            output::print(format, &requirement)?;
        }
        RequirementsCommand::SourceRef { command } => source_ref(command)?,
        RequirementsCommand::Refines { command } => {
            references::requirement_single(RequirementSingle::Refines, command)?;
        }
        RequirementsCommand::DependsOn { command } => {
            references::requirement_list(RequirementList::DependsOn, command)?;
        }
        RequirementsCommand::Supersedes { command } => {
            references::requirement_list(RequirementList::Supersedes, command)?;
        }
        RequirementsCommand::SpawnedBy { command } => {
            references::requirement_single(RequirementSingle::SpawnedBy, command)?;
        }
        RequirementsCommand::Fog { command } => fog(command)?,
    }
    Ok(())
}

fn source_ref(command: SourceRefCommand) -> anyhow::Result<()> {
    match command {
        SourceRefCommand::Add {
            repo,
            scope,
            requirement_id,
            source_id,
            clause,
            format,
        } => {
            let requirement = StateStore::new(ProvenanceLayout::new(repo)).add_source_reference(
                AddSourceReferenceInput {
                    scope_id: ScopeId::new(scope)?,
                    source_id: StableId::new(source_id)?,
                    requirement_id: StableId::new(requirement_id)?,
                    clause,
                },
            )?;
            output::print(format, &requirement)?;
        }
        SourceRefCommand::Clear {
            repo,
            scope,
            requirement_id,
            source_id,
            format,
        } => {
            let requirement = StateStore::new(ProvenanceLayout::new(repo)).clear_source_reference(
                &ScopeId::new(scope)?,
                &StableId::new(requirement_id)?,
                &StableId::new(source_id)?,
            )?;
            output::print(format, &requirement)?;
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
            format,
        } => {
            let requirement = StateStore::new(ProvenanceLayout::new(repo)).set_requirement_fog(
                &ScopeId::new(scope)?,
                &StableId::new(requirement_id)?,
                Some(text),
            )?;
            output::print(format, &requirement)?;
        }
        FogCommand::Show {
            repo,
            scope,
            requirement_id,
            format,
        } => {
            let requirement_id = StableId::new(requirement_id)?;
            let requirement = StateStore::new(ProvenanceLayout::new(repo))
                .list_requirements(&ScopeId::new(scope)?)?
                .into_iter()
                .find(|requirement| requirement.id == requirement_id)
                .ok_or_else(|| anyhow::anyhow!("requirement does not exist"))?;
            output::print(
                format,
                &FogView {
                    requirement_id: requirement.id.as_str().to_string(),
                    fog: requirement.fog,
                },
            )?;
        }
        FogCommand::Clear {
            repo,
            scope,
            requirement_id,
            format,
        } => {
            let requirement = StateStore::new(ProvenanceLayout::new(repo)).set_requirement_fog(
                &ScopeId::new(scope)?,
                &StableId::new(requirement_id)?,
                None,
            )?;
            output::print(format, &requirement)?;
        }
    }
    Ok(())
}
