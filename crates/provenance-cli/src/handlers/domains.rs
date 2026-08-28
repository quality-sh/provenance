use crate::cli::knowledge::DomainsCommand;
use crate::output;
use provenance_core::{ScopeId, StableId};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{CreateDomainInput, StateStore},
};

pub(super) fn handle(
    command: DomainsCommand,
    actor_claim: Option<&provenance_core::RbacClaim>,
) -> anyhow::Result<()> {
    match command {
        DomainsCommand::Create {
            repo,
            scope,
            id,
            name,
            description,
            color,
            format,
        } => {
            let domain = StateStore::new(ProvenanceLayout::new(repo)).create_domain(
                actor_claim,
                CreateDomainInput {
                    scope_id: ScopeId::new(scope)?,
                    id: StableId::new(id)?,
                    name,
                    description,
                    color,
                },
            )?;
            output::print(format, &domain)?;
        }
        DomainsCommand::List {
            repo,
            scope,
            format,
        } => {
            let domains =
                StateStore::new(ProvenanceLayout::new(repo)).list_domains(&ScopeId::new(scope)?)?;
            output::print(format, &domains)?;
        }
    }
    Ok(())
}
