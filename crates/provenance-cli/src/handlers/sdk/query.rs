use crate::cli::sdk::QueryArgs;
use crate::output;
use provenance_core::protocol::QueryResponse;
use provenance_core::ScopeId;
use provenance_store::operations::queries;

/// Which structured query the caller asked for.
#[derive(Debug, Clone, Copy)]
pub(super) enum Operation {
    Get,
    Search,
    Neighbors,
    Trace,
    Impact,
    Evidence,
    Stale,
    ResolveSymbol,
}

/// Runs one structured query and writes its bounded answer.
///
/// Every primitive is one named operation with typed parameters read from
/// stdin, and every answer carries the protocol version that produced it.
pub(super) async fn handle(operation: Operation, args: QueryArgs) -> anyhow::Result<()> {
    let repo = Some(provenance_store::operations::discover_repository(
        args.repo,
    )?);
    let scope = ScopeId::new(args.scope)?;
    let format = args.format;
    match operation {
        Operation::Get => {
            let result = queries::get(repo, &scope, super::read_stdin_json()?).await?;
            output::print(format, &QueryResponse::new("get", result))
        }
        Operation::Search => {
            let result = queries::search(repo, &scope, super::read_stdin_json()?).await?;
            output::print(format, &QueryResponse::new("search", result))
        }
        Operation::Neighbors => {
            let result = queries::neighbors(repo, &scope, super::read_stdin_json()?).await?;
            output::print(format, &QueryResponse::new("neighbors", result))
        }
        Operation::Trace => {
            let result = queries::trace(repo, &scope, super::read_stdin_json()?).await?;
            output::print(format, &QueryResponse::new("trace", result))
        }
        Operation::Impact => {
            let result = queries::impact(repo, &scope, super::read_stdin_json()?).await?;
            output::print(format, &QueryResponse::new("impact", result))
        }
        Operation::Evidence => {
            let result = queries::evidence(repo, &scope, super::read_stdin_json()?).await?;
            output::print(format, &QueryResponse::new("evidence", result))
        }
        Operation::Stale => {
            let result = queries::stale(repo, &scope, super::read_stdin_json()?)?;
            output::print(format, &QueryResponse::new("stale", result))
        }
        Operation::ResolveSymbol => {
            let result = queries::resolve_symbol(repo, &scope, super::read_stdin_json()?).await?;
            output::print(format, &QueryResponse::new("resolve-symbol", result))
        }
    }
}
