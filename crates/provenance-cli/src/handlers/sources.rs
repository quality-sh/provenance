use super::common::stable_ids;
use super::refs;
use crate::cli::knowledge::SourcesCommand;
use crate::output;
use crate::store::Store;
use provenance_core::{ScopeId, SourceType, StableId};
use provenance_store::state_store::CreateSourceInput;

pub(super) async fn handle(command: SourcesCommand) -> anyhow::Result<()> {
    match command {
        SourcesCommand::Update(args) => {
            super::updates::handle::<provenance_store::operations::catalog::UpdateSource>(args)
                .await?;
        }
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
            ..
        } => {
            let source = Store::open(repo).create_source(CreateSourceInput {
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
            output::print_json(&source)?;
        }
        SourcesCommand::Supersedes { command } => refs::source_supersedes(command).await?,
    }
    Ok(())
}
