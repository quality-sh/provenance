use std::io::Read as _;

use crate::cli::sdk::SdkCommand;
use crate::output::{self, ReportFormat};
use provenance_store::operations;
use provenance_store::state_store::{BeginVerificationInput, CompleteVerificationInput};

mod authoring;
mod check_statement;
mod query;
mod render;
mod verification_lists;

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
            let repo = operations::discover_repository(repo)?;
            let input = read_stdin_json()?;
            let plan = authoring::invoke::<operations::catalog::Plan>(repo, scope, input).await?;
            match format {
                ReportFormat::Json => output::print_json(&plan)?,
                ReportFormat::Markdown => print!("{}", render::render(&plan)),
            }
        }
        SdkCommand::Apply { repo, scope, .. } => {
            let repo = operations::discover_repository(repo)?;
            let input = read_stdin_json()?;
            let result =
                authoring::invoke::<operations::catalog::Apply>(repo, scope, input).await?;
            output::print_json(&result)?;
        }
        SdkCommand::BeginVerification { repo, scope, .. } => {
            let repo = operations::discover_repository(repo)?;
            let input = read_stdin_json::<BeginVerificationInput>()?;
            let run =
                authoring::invoke::<operations::catalog::BeginVerification>(repo, scope, input)
                    .await?;
            output::print_json(&run)?;
        }
        SdkCommand::CompleteVerification { repo, scope, .. } => {
            let repo = operations::discover_repository(repo)?;
            let input = read_stdin_json::<CompleteVerificationInput>()?;
            let run =
                authoring::invoke::<operations::catalog::CompleteVerification>(repo, scope, input)
                    .await?;
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
