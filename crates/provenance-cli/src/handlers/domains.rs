use crate::cli::knowledge::DomainsCommand;
use crate::output;
use crate::store::Store;
use provenance_core::{ScopeId, StableId};
use provenance_store::state_store::CreateDomainInput;

pub(super) async fn handle(command: DomainsCommand) -> anyhow::Result<()> {
    match command {
        DomainsCommand::Update(args) => {
            super::updates::handle::<provenance_store::operations::catalog::UpdateDomain>(args)
                .await?;
        }
        DomainsCommand::Create {
            repo,
            scope,
            id,
            name,
            description,
            color,
            ..
        } => {
            let domain = Store::open(repo).create_domain(CreateDomainInput {
                scope_id: ScopeId::new(scope)?,
                id: StableId::new(id)?,
                name,
                description,
                color,
            })?;
            output::print_json(&domain)?;
        }
        DomainsCommand::List { repo, scope, .. } => {
            let domains = Store::open(repo).list_domains(&ScopeId::new(scope)?)?;
            output::print_json(&domains)?;
        }
    }
    Ok(())
}
