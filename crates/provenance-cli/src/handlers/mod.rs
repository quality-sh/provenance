use crate::cli::Command;

mod cargo_init;
pub mod check;
mod coverage;
mod dictionary;
mod docs;
#[cfg(feature = "dogfood")]
mod dogfood;
mod export;
mod gaps;
mod graph;
mod graph_reference;
mod health;
mod import;
mod materialize;
mod merge_jsonl;
mod orphans;
mod prime;
mod repo;
mod report;
mod schema;
mod skills;
mod swarm_backtrace;
mod traceability;
mod validate;
mod wiki;

#[allow(clippy::redundant_pub_crate)]
pub(super) use export::{export_scope, ScopeExport};

/// Runs one built-in command. The commands that wait on async work or on a
/// blocking thread run here. Every other command runs on the calling thread.
#[allow(clippy::redundant_pub_crate)]
pub(super) async fn dispatch(command: Command, quiet: bool) -> anyhow::Result<()> {
    match command {
        Command::Search(args) => args.dispatch().await,
        Command::CargoInit { package, ste_pdf } => {
            run_blocking(move || cargo_init::handle(package.as_deref(), ste_pdf, quiet)).await
        }
        Command::Init {
            path,
            scope,
            path_prefix,
            disposition_actor_id,
            clear_disposition_actors,
            ste_pdf,
            invocation_channel,
            package_manager,
        } => {
            let options = repo::InitOptions {
                scope,
                path_prefix,
                disposition_actor_ids: disposition_actor_id,
                clear_disposition_actors,
                ste_pdf,
                invocation_channel,
                package_manager,
                quiet,
            };
            run_blocking(move || repo::init(&path, options)).await
        }
        Command::Check {
            repo,
            strict,
            base,
            graph,
            statements,
            bindings,
            format,
        } => {
            let selectors = check::Selectors {
                graph,
                statements,
                bindings,
            };
            check::check(repo, strict, base, format.is_some(), selectors).await
        }
        Command::Docs { command } => docs::handle(command).await,
        Command::Wiki { command } => wiki::handle(command).await,
        Command::Review(options) => crate::review::run(options).await,
        Command::Materialize { repo, .. } => materialize::handle(repo).await,
        command => dispatch_on_thread(command, quiet),
    }
}

/// Runs a synchronous handler on a blocking thread and waits for it.
async fn run_blocking(
    handler: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(handler).await?
}

/// Runs one built-in command whose handler does its work on the calling
/// thread.
#[cfg_attr(not(feature = "dogfood"), allow(unused_variables))]
fn dispatch_on_thread(command: Command, quiet: bool) -> anyhow::Result<()> {
    match command {
        Command::Dictionary { command } => dictionary::handle(command),
        Command::GraphReference { command } => graph_reference::handle(command),
        Command::Graph {
            requirement_id,
            repo,
            scope,
            ..
        } => graph::handle(requirement_id, repo, scope),
        Command::Traceability {
            rule_id,
            repo,
            scope,
            ..
        } => traceability::handle(rule_id, repo, scope),
        Command::Gaps { repo, scope, .. } => gaps::handle(repo, scope),
        Command::Prime { format, .. } => prime::handle(format),
        Command::Health { repo, scope, .. } => health::handle(repo, scope),
        Command::Orphans { repo, scope, .. } => orphans::handle(repo, scope),
        Command::Coverage { command } => coverage::handle(command),
        Command::Report { command } => report::handle(command),
        Command::SwarmBacktrace { command } => swarm_backtrace::handle(command),
        Command::Skills { command } => skills::handle(command),
        Command::Schema { command } => schema::handle(command),
        Command::Validate {
            artifact, input, ..
        } => validate::handle(artifact, &input),
        Command::Export {
            repo,
            scope,
            format,
            output,
        } => export::handle(repo, scope, format, output),
        Command::Import {
            repo,
            scope,
            input,
            dry_run,
            ..
        } => import::handle(repo, scope, input, dry_run),
        Command::MergeJsonl {
            base,
            ours,
            theirs,
            output,
            path,
            ..
        } => merge_jsonl::handle(&base, &ours, &theirs, output, path.as_deref()),
        #[cfg(feature = "dogfood")]
        Command::Dogfood { command } => dogfood::handle(command, quiet),
        Command::Search(_)
        | Command::CargoInit { .. }
        | Command::Init { .. }
        | Command::Check { .. }
        | Command::Docs { .. }
        | Command::Wiki { .. }
        | Command::Review(_)
        | Command::Materialize { .. } => {
            unreachable!("dispatch runs the async and blocking-thread commands")
        }
    }
}
