use crate::cli::knowledge::DomainsCommand;
use crate::output;
use provenance_core::{ScopeId, StableId};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{CreateDomainInput, StateStore},
};

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
            format,
        } => {
            let domain =
                StateStore::new(ProvenanceLayout::new(repo)).create_domain(CreateDomainInput {
                    scope_id: ScopeId::new(scope)?,
                    id: StableId::new(id)?,
                    name,
                    description,
                    color,
                })?;
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
