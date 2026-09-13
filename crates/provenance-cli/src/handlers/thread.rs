use crate::cli::shaping::ThreadCommand;
use crate::output;
use crate::store::Store;
use provenance_core::{MessageRole, NodeType, ScopeId, StableId, ThreadParent};
use provenance_store::state_store::PostMessageInput;

pub(super) fn handle(command: ThreadCommand) -> anyhow::Result<()> {
    match command {
        ThreadCommand::Post {
            repo,
            scope,
            parent_type,
            parent_id,
            role,
            body,
            ..
        } => {
            let result = Store::open(repo).post_thread_message(PostMessageInput {
                scope_id: ScopeId::new(scope)?,
                parent: ThreadParent {
                    node_type: NodeType::parse(&parent_type)?,
                    node_id: StableId::new(parent_id)?,
                },
                role: MessageRole::parse(&role)?,
                body,
            })?;
            output::print_json(&result)?;
        }
        ThreadCommand::List { repo, scope, .. } => {
            let threads = Store::open(repo).list_threads(&ScopeId::new(scope)?)?;
            output::print_json(&threads)?;
        }
    }
    Ok(())
}
