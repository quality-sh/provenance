use super::common::boundary_source_ref;
use crate::cli::knowledge::BoundariesCommand;
use crate::output;
use provenance_core::{ScopeId, StableId};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{CreateBoundaryInput, StateStore},
};

pub(super) fn handle(
    command: BoundariesCommand,
    actor_claim: Option<&provenance_core::RbacClaim>,
) -> anyhow::Result<()> {
    match command {
        BoundariesCommand::Create {
            repo,
            scope,
            id,
            requirement_id,
            statement,
            source_id,
            source_clause,
            format,
        } => {
            let boundary = StateStore::new(ProvenanceLayout::new(repo)).create_boundary(
                actor_claim,
                CreateBoundaryInput {
                    scope_id: ScopeId::new(scope)?,
                    id: StableId::new(id)?,
                    requirement_id: StableId::new(requirement_id)?,
                    statement,
                    source_ref: boundary_source_ref(source_id, source_clause)?,
                },
            )?;
            output::print(format, &boundary)?;
        }
        BoundariesCommand::List {
            repo,
            scope,
            format,
        } => {
            let boundaries = StateStore::new(ProvenanceLayout::new(repo))
                .list_boundaries(&ScopeId::new(scope)?)?;
            output::print(format, &boundaries)?;
        }
    }
    Ok(())
}
