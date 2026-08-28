use crate::cli::shaping::ThreadCommand;
use crate::output;
use provenance_core::{MessageRole, NodeType, ScopeId, StableId, ThreadParent};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{PostMessageInput, StateStore},
};

pub(super) fn handle(
    command: ThreadCommand,
    actor_claim: Option<&provenance_core::RbacClaim>,
) -> anyhow::Result<()> {
    match command {
        ThreadCommand::Post {
            repo,
            scope,
            parent_type,
            parent_id,
            role,
            body,
            format,
        } => {
            let result = StateStore::new(ProvenanceLayout::new(repo)).post_thread_message(
                actor_claim,
                PostMessageInput {
                    scope_id: ScopeId::new(scope)?,
                    parent: ThreadParent {
                        node_type: NodeType::parse(&parent_type)?,
                        node_id: StableId::new(parent_id)?,
                    },
                    role: MessageRole::parse(&role)?,
                    body,
                },
            )?;
            output::print(format, &result)?;
        }
        ThreadCommand::List {
            repo,
            scope,
            format,
        } => {
            let threads =
                StateStore::new(ProvenanceLayout::new(repo)).list_threads(&ScopeId::new(scope)?)?;
            output::print(format, &threads)?;
        }
    }
    Ok(())
}
