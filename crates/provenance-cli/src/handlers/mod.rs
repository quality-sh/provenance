use crate::cli::Command;
use crate::output;
use provenance_core::StableId;
use provenance_store::cache;

mod cargo_init;
pub mod check;
mod coverage;
mod dictionary;
mod docs;
#[cfg(feature = "dogfood")]
mod dogfood;
mod export;
mod graph_reference;
mod import;
mod merge_jsonl;
mod prime;
mod repo;
mod report;
mod schema;
mod skills;
mod swarm_backtrace;
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
            let context = provenance_cli::repo_context::RepoContext::new(repo, "default");
            check::check(context, strict, base, format.is_some(), selectors).await
        }
        Command::Docs { command } => docs::handle(command).await,
        Command::Wiki { command } => wiki::handle(command).await,
        Command::Review(options) => crate::review::run(options).await,
        Command::Materialize { repo, .. } => {
            let store = crate::store::Store::open_required(repo)?;
            output::print_json(&cache::materialize_state(store.layout()).await?)
        }
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
        Command::Graph {
            requirement_id,
            context,
            ..
        } => {
            let store = context.open_graph()?;
            let graph = cache::get_requirement_graph(
                store.layout(),
                &context.scope_id()?,
                &StableId::new(requirement_id)?,
            )?;
            output::print_json(&graph)
        }
        Command::Traceability {
            rule_id, context, ..
        } => {
            let store = context.open_graph()?;
            let trace = cache::trace_rule(
                store.layout(),
                &context.scope_id()?,
                &StableId::new(rule_id)?,
            )?;
            output::print_json(&trace)
        }
        Command::Gaps { context, .. } => {
            let store = context.open_graph()?;
            output::print_json(&cache::find_gaps(store.layout(), &context.scope_id()?)?)
        }
        Command::Prime { format, .. } => prime::handle(format),
        Command::Health { context, .. } => {
            let store = context.open_graph()?;
            output::print_json(&cache::coverage_health(
                store.layout(),
                &context.scope_id()?,
            )?)
        }
        Command::Orphans { context, .. } => {
            let store = context.open_graph()?;
            output::print_json(&cache::orphan_rules(store.layout(), &context.scope_id()?)?)
        }
        Command::Export {
            context,
            format,
            output,
        } => export::handle(&context, format, output),
        Command::Import {
            context,
            input,
            dry_run,
            ..
        } => import::handle(&context, input, dry_run),
        command => dispatch_remaining_on_thread(command, quiet),
    }
}

/// Runs a synchronous command that does not use the shared repository context.
#[cfg_attr(not(feature = "dogfood"), allow(unused_variables))]
fn dispatch_remaining_on_thread(command: Command, quiet: bool) -> anyhow::Result<()> {
    match command {
        Command::Dictionary { command } => dictionary::handle(command),
        Command::GraphReference { command } => graph_reference::handle(command),
        Command::Coverage { command } => coverage::handle(command),
        Command::Report { command } => report::handle(command),
        Command::SwarmBacktrace { command } => swarm_backtrace::handle(command),
        Command::Skills { command } => skills::handle(command),
        Command::Schema { command } => schema::handle(command),
        Command::Validate {
            artifact, input, ..
        } => validate::handle(artifact, &input),
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
        // These commands run in `dispatch` or `dispatch_on_thread`.
        Command::Search(_)
        | Command::CargoInit { .. }
        | Command::Init { .. }
        | Command::Check { .. }
        | Command::Docs { .. }
        | Command::Wiki { .. }
        | Command::Review(_)
        | Command::Materialize { .. }
        | Command::Graph { .. }
        | Command::Traceability { .. }
        | Command::Gaps { .. }
        | Command::Prime { .. }
        | Command::Health { .. }
        | Command::Orphans { .. }
        | Command::Export { .. }
        | Command::Import { .. } => {
            unreachable!("an earlier dispatcher runs this command")
        }
    }
}
