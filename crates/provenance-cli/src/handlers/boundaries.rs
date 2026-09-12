use super::common::boundary_source_ref;
use crate::cli::knowledge::BoundariesCommand;
use crate::output;
use crate::store::Store;
use provenance_core::{ScopeId, StableId};
use provenance_store::state_store::CreateBoundaryInput;

pub(super) async fn handle(command: BoundariesCommand) -> anyhow::Result<()> {
    match command {
        BoundariesCommand::Update(args) => {
            super::updates::handle::<provenance_store::operations::catalog::UpdateBoundary>(args)
                .await?;
        }
        BoundariesCommand::Create {
            repo,
            scope,
            id,
            requirement_id,
            statement,
            source_id,
            source_clause,
            ..
        } => {
            let boundary = Store::open(repo).create_boundary(CreateBoundaryInput {
                scope_id: ScopeId::new(scope)?,
                id: StableId::new(id)?,
                requirement_id: StableId::new(requirement_id)?,
                statement,
                source_ref: boundary_source_ref(source_id, source_clause)?,
            })?;
            output::print_json(&boundary)?;
        }
        BoundariesCommand::List { repo, scope, .. } => {
            let boundaries = Store::open(repo).list_boundaries(&ScopeId::new(scope)?)?;
            output::print_json(&boundaries)?;
        }
    }
    Ok(())
}
