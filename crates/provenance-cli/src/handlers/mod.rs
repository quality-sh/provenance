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

#[allow(clippy::too_many_lines)]
#[allow(clippy::redundant_pub_crate)]
pub(super) async fn dispatch(command: Command, quiet: bool) -> anyhow::Result<()> {
    let _ = quiet;
    match command {
        Command::CargoInit { package, ste_pdf } => {
            tokio::task::spawn_blocking(move || cargo_init::handle(package.as_deref(), ste_pdf))
                .await??;
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
            tokio::task::spawn_blocking(move || {
                repo::init(
                    &path,
                    repo::InitOptions {
                        scope,
                        path_prefix,
                        disposition_actor_ids: disposition_actor_id,
                        clear_disposition_actors,
                        ste_pdf,
                        invocation_channel,
                        package_manager,
                    },
                )
            })
            .await??;
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
            check::check(
                repo,
                strict,
                base,
                format.is_some(),
                check::Selectors {
                    graph,
                    statements,
                    bindings,
                },
            )
            .await?;
        }
        Command::Docs { command } => {
            docs::handle(command).await?;
        }
        Command::Dictionary { command } => {
            dictionary::handle(command)?;
        }
        Command::Wiki { command } => {
            wiki::handle(command).await?;
        }
        Command::Review(options) => crate::review::run(options).await?,
        Command::Materialize { repo, .. } => {
            materialize::handle(repo).await?;
        }
        Command::GraphReference { command } => {
            graph_reference::handle(command)?;
        }
        Command::Graph {
            requirement_id,
            repo,
            scope,
            ..
        } => {
            graph::handle(requirement_id, repo, scope)?;
        }
        Command::Traceability {
            rule_id,
            repo,
            scope,
            ..
        } => {
            traceability::handle(rule_id, repo, scope)?;
        }
        Command::Gaps { repo, scope, .. } => {
            gaps::handle(repo, scope)?;
        }
        Command::Prime {
            repo,
            scope,
            format,
            include_threads,
        } => {
            prime::handle(repo, scope, format, include_threads)?;
        }
        Command::Health { repo, scope, .. } => {
            health::handle(repo, scope)?;
        }
        Command::Orphans { repo, scope, .. } => {
            orphans::handle(repo, scope)?;
        }
        Command::Coverage { command } => {
            coverage::handle(command)?;
        }
        Command::Report { command } => {
            report::handle(command)?;
        }
        Command::SwarmBacktrace { command } => {
            swarm_backtrace::handle(command)?;
        }
        Command::Skills { command } => {
            skills::handle(command)?;
        }
        Command::Schema { command } => {
            schema::handle(command)?;
        }
        Command::Validate {
            artifact, input, ..
        } => {
            validate::handle(artifact, &input)?;
        }
        Command::Export {
            repo,
            scope,
            format,
            output,
        } => {
            export::handle(repo, scope, format, output)?;
        }
        Command::Import {
            repo,
            scope,
            input,
            dry_run,
            ..
        } => {
            import::handle(repo, scope, input, dry_run)?;
        }
        Command::MergeJsonl {
            base,
            ours,
            theirs,
            output,
            path,
            ..
        } => {
            merge_jsonl::handle(&base, &ours, &theirs, output, path.as_deref())?;
        }
        #[cfg(feature = "dogfood")]
        Command::Dogfood { command } => {
            dogfood::handle(command, quiet)?;
        }
    }
    Ok(())
}
