use crate::cli::knowledge::SourcesCommand;
use crate::output;
use provenance_core::{ScopeId, SourceType, StableId};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{CreateSourceInput, StateStore},
};

pub(super) fn handle(
    command: SourcesCommand,
    actor_claim: Option<&provenance_core::RbacClaim>,
) -> anyhow::Result<()> {
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
            superseded_by,
            origin_thread,
            origin_message,
            format,
        } => {
            let source = StateStore::new(ProvenanceLayout::new(repo)).create_source(
                actor_claim,
                CreateSourceInput {
                    scope_id: ScopeId::new(scope)?,
                    id: StableId::new(id)?,
                    name,
                    source_type: SourceType::parse(&source_type)?,
                    url,
                    reference,
                    commit_pin,
                    effective_date,
                    review_date,
                    superseded_by: superseded_by.map(StableId::new).transpose()?,
                    origin_thread: origin_thread.map(StableId::new).transpose()?,
                    origin_message: origin_message.map(StableId::new).transpose()?,
                },
            )?;
            output::print(format, &source)?;
        }
    }
    Ok(())
}
