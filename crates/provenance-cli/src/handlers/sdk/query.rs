use crate::cli::sdk::QueryArgs;
use crate::output;
use provenance_core::protocol::QueryResponse;
use provenance_core::ScopeId;
use provenance_store::operations::{catalog, queries};

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
    let root = provenance_store::operations::discover_repository(args.repo)?;
    let settings = provenance_store::settings::Settings::load(
        &provenance_store::layout::ProvenanceLayout::new(root.clone()),
    )?;
    let policy =
        provenance_store::operations::read_policy::ReadPolicy::resolve(&settings, args.freshness);
    let repo = Some(root.clone());
    let scope = ScopeId::new(args.scope)?;
    let format = args.format;
    let context = catalog::PreparedContext::read(catalog::PreparedRead {
        root,
        scope: scope.clone(),
        policy,
        requested_target: String::new(),
        external: false,
    });
    match operation {
        Operation::Get => {
            let result =
                catalog::invoke_typed::<catalog::Get>(context, super::read_stdin_json()?).await?;
            output::print(format, &result)
        }
        Operation::Search => {
            let result =
                catalog::invoke_typed::<catalog::Search>(context, super::read_stdin_json()?)
                    .await?;
            output::print(format, &result)
        }
        Operation::Neighbors => {
            let result =
                catalog::invoke_typed::<catalog::Neighbors>(context, super::read_stdin_json()?)
                    .await?;
            output::print(format, &result)
        }
        Operation::Trace => {
            let result =
                catalog::invoke_typed::<catalog::Trace>(context, super::read_stdin_json()?).await?;
            output::print(format, &result)
        }
        Operation::Impact => {
            let result = queries::impact(repo, &scope, policy, super::read_stdin_json()?).await?;
            output::print(format, &QueryResponse::new("impact", result))
        }
        Operation::Evidence => {
            let result = queries::evidence(repo, &scope, policy, super::read_stdin_json()?).await?;
            output::print(format, &QueryResponse::new("evidence", result))
        }
        Operation::Stale => {
            let result = queries::stale(repo, &scope, policy, super::read_stdin_json()?).await?;
            output::print(format, &QueryResponse::new("stale", result))
        }
        Operation::ResolveSymbol => {
            let result =
                queries::resolve_symbol(repo, &scope, policy, super::read_stdin_json()?).await?;
            output::print(format, &QueryResponse::new("resolve-symbol", result))
        }
    }
}
