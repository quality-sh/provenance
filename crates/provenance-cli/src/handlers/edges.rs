use crate::cli::graph::EdgesCommand;
use crate::output;
use provenance_core::{EdgeType, NodeType, ScopeId, StableId};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{CreateEdgeInput, StateStore},
};

pub(super) fn handle(
    command: EdgesCommand,
    actor_claim: Option<&provenance_core::RbacClaim>,
) -> anyhow::Result<()> {
    match command {
        EdgesCommand::Create {
            repo,
            scope,
            edge_type,
            from_type,
            from_id,
            to_type,
            to_id,
            format,
        } => {
            let edge = StateStore::new(ProvenanceLayout::new(repo)).create_edge(
                actor_claim,
                CreateEdgeInput {
                    scope_id: ScopeId::new(scope)?,
                    edge_type: EdgeType::parse(&edge_type)?,
                    from_type: NodeType::parse(&from_type)?,
                    from_id: StableId::new(from_id)?,
                    to_type: NodeType::parse(&to_type)?,
                    to_id: StableId::new(to_id)?,
                },
            )?;
            output::print(format, &edge)?;
        }
        EdgesCommand::List {
            repo,
            scope,
            format,
        } => {
            let scope_id = ScopeId::new(scope)?;
            let edges: Vec<_> = StateStore::new(ProvenanceLayout::new(repo))
                .list_edges()?
                .into_iter()
                .filter(|edge| edge.scope_id == scope_id)
                .collect();
            output::print(format, &edges)?;
        }
        EdgesCommand::Delete {
            repo,
            scope,
            id,
            format,
        } => {
            let edge = StateStore::new(ProvenanceLayout::new(repo)).delete_edge(
                actor_claim,
                &ScopeId::new(scope)?,
                &StableId::new(id)?,
            )?;
            output::print(format, &edge)?;
        }
    }
    Ok(())
}
