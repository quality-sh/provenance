use std::io::Read as _;

use crate::cli::sdk::SdkCommand;
use crate::output::{self, ReportFormat};
use camino::Utf8PathBuf;
use provenance_core::ScopeId;
use provenance_store::operations::{self, catalog};

mod check_statement;
mod query;
mod render;
mod verification_lists;

/// Reads one request document from stdin and runs it as a native catalog
/// operation on the selected repository and scope.
async fn submit<O>(repo: Option<Utf8PathBuf>, scope: String) -> anyhow::Result<O::Success>
where
    O: catalog::Operation,
    O::Failure: Sync,
{
    let repo = operations::discover_repository(repo)?;
    let input = read_stdin_json::<O::Request>()?;
    super::native::invoke_native::<O>(repo, ScopeId::new(scope)?, input).await
}

pub(super) async fn handle(command: SdkCommand) -> anyhow::Result<()> {
    match command {
        SdkCommand::CheckStatement { .. } => check_statement::handle().await?,
        SdkCommand::Info { repo, .. } => {
            output::print_json(&operations::engine_info(repo)?)?;
        }
        SdkCommand::Plan {
            repo,
            scope,
            format,
        } => {
            let plan = submit::<operations::catalog::Plan>(repo, scope).await?;
            match format {
                ReportFormat::Json => output::print_json(&plan)?,
                ReportFormat::Markdown => print!("{}", render::render(&plan)),
            }
        }
        SdkCommand::Apply { repo, scope, .. } => {
            let result = submit::<operations::catalog::Apply>(repo, scope).await?;
            output::print_json(&result)?;
        }
        SdkCommand::BeginVerification { repo, scope, .. } => {
            let run = submit::<operations::catalog::BeginVerification>(repo, scope).await?;
            output::print_json(&run)?;
        }
        SdkCommand::CompleteVerification { repo, scope, .. } => {
            let run = submit::<operations::catalog::CompleteVerification>(repo, scope).await?;
            output::print_json(&run)?;
        }
        SdkCommand::VerificationRuns {
            repo, scope, rule, ..
        } => {
            verification_lists::print::<operations::catalog::VerificationRuns>(repo, scope, rule)
                .await?;
        }
        SdkCommand::Get { query } => query::handle(query::Operation::Get, query).await?,
        SdkCommand::Search { query } => query::handle(query::Operation::Search, query).await?,
        SdkCommand::Neighbors { query } => {
            query::handle(query::Operation::Neighbors, query).await?;
        }
        SdkCommand::Trace { query } => query::handle(query::Operation::Trace, query).await?,
        SdkCommand::Impact { query } => query::handle(query::Operation::Impact, query).await?,
        SdkCommand::Evidence { query } => query::handle(query::Operation::Evidence, query).await?,
        SdkCommand::Stale { query } => query::handle(query::Operation::Stale, query).await?,
        SdkCommand::ResolveSymbol { query } => {
            query::handle(query::Operation::ResolveSymbol, query).await?;
        }
        SdkCommand::VerificationBindings {
            repo, scope, rule, ..
        } => {
            verification_lists::print::<operations::catalog::VerificationBindings>(
                repo, scope, rule,
            )
            .await?;
        }
    }
    Ok(())
}

fn read_stdin_json<T: serde::de::DeserializeOwned>() -> anyhow::Result<T> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    anyhow::ensure!(
        !input.trim().is_empty(),
        "expected a JSON document on stdin"
    );
    serde_json::from_str(&input).map_err(Into::into)
}
