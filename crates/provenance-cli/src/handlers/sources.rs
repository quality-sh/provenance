use super::common::stable_ids;
use super::references;
use crate::cli::knowledge::SourcesCommand;
use crate::output;
use provenance_core::{ScopeId, SourceType, StableId};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{CreateSourceInput, StateStore},
};

pub(super) fn handle(command: SourcesCommand) -> anyhow::Result<()> {
    match command {
        SourcesCommand::Create {
            repo,
            scope,
            id,
            name,
            source_type,
            url,
            reference,
            commit_pin,
            effective_date,
            review_date,
            supersedes,
            origin_thread,
            origin_message,
            format,
        } => {
            let source =
                StateStore::new(ProvenanceLayout::new(repo)).create_source(CreateSourceInput {
                    scope_id: ScopeId::new(scope)?,
                    id: StableId::new(id)?,
                    name,
                    source_type: SourceType::parse(&source_type)?,
                    url,
                    reference,
                    commit_pin,
                    effective_date,
                    review_date,
                    supersedes: stable_ids(supersedes)?,
                    origin_thread: origin_thread.map(StableId::new).transpose()?,
                    origin_message: origin_message.map(StableId::new).transpose()?,
                })?;
            output::print(format, &source)?;
        }
        SourcesCommand::Supersedes { command } => references::source_supersedes(command)?,
    }
    Ok(())
}
