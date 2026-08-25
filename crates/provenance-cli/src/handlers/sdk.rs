use std::io::Read as _;

use crate::cli::sdk::SdkCommand;
use crate::output;
use provenance_core::{ScopeId, StableId};
use provenance_store::operations;
use provenance_store::state_store::{BeginVerificationInput, CompleteVerificationInput};

mod check_statement;
mod query;
mod render;

pub(super) fn handle(command: SdkCommand) -> anyhow::Result<()> {
    match command {
        SdkCommand::CheckStatement { format } => check_statement::handle(format)?,
        SdkCommand::Info { repo, format } => {
            output::print(format, &operations::engine_info(repo)?)?;
        }
        SdkCommand::Plan {
            repo,
            scope,
            format,
        } => {
            let repo = Some(operations::discover_repository(repo)?);
            let input = read_stdin_json()?;
            let plan = operations::plan(repo, &ScopeId::new(scope)?, input)?;
            match format {
                output::OutputFormat::Json | output::OutputFormat::Jsonl => {
                    output::print(format, &plan)?;
                }
                output::OutputFormat::Markdown
                | output::OutputFormat::Table
                | output::OutputFormat::Toon => {
                    print!("{}", render::render(&plan));
                }
            }
        }
        SdkCommand::Apply {
            repo,
            scope,
            format,
        } => {
            let repo = Some(operations::discover_repository(repo)?);
            let input = read_stdin_json()?;
            let result = operations::apply(repo, &ScopeId::new(scope)?, input)?;
            output::print(format, &result)?;
        }
        SdkCommand::BeginVerification {
            repo,
            scope,
            format,
        } => {
            let repo = Some(operations::discover_repository(repo)?);
            let input = read_stdin_json::<BeginVerificationInput>()?;
            let run = operations::begin_verification(repo, ScopeId::new(scope)?, input)?;
            output::print(format, &run)?;
        }
        SdkCommand::CompleteVerification {
            repo,
            scope,
            format,
        } => {
            let repo = Some(operations::discover_repository(repo)?);
            let input = read_stdin_json::<CompleteVerificationInput>()?;
            let run = operations::complete_verification(repo, &ScopeId::new(scope)?, input)?;
            output::print(format, &run)?;
        }
        SdkCommand::VerificationRuns {
            repo,
            scope,
            rule,
            format,
        } => {
            let repo = Some(operations::discover_repository(repo)?);
            let rule = rule.map(StableId::new).transpose()?;
            let runs = operations::verification_runs(repo, &ScopeId::new(scope)?, rule.as_ref())?;
            output::print(format, &runs)?;
        }
        SdkCommand::Get { query } => query::handle(query::Operation::Get, query)?,
        SdkCommand::Search { query } => query::handle(query::Operation::Search, query)?,
        SdkCommand::Neighbors { query } => query::handle(query::Operation::Neighbors, query)?,
        SdkCommand::Trace { query } => query::handle(query::Operation::Trace, query)?,
        SdkCommand::Impact { query } => query::handle(query::Operation::Impact, query)?,
        SdkCommand::Evidence { query } => query::handle(query::Operation::Evidence, query)?,
        SdkCommand::Stale { query } => query::handle(query::Operation::Stale, query)?,
        SdkCommand::ResolveSymbol { query } => {
            query::handle(query::Operation::ResolveSymbol, query)?;
        }
        SdkCommand::VerificationBindings {
            repo,
            scope,
            rule,
            format,
        } => {
            let repo = Some(operations::discover_repository(repo)?);
            let rule = rule.map(StableId::new).transpose()?;
            let bindings =
                operations::verification_bindings(repo, &ScopeId::new(scope)?, rule.as_ref())?;
            output::print(format, &bindings)?;
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
