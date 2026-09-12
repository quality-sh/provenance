use crate::cli::Command;

mod boundaries;
mod cargo_init;
mod check;
mod common;
mod contributions;
mod coverage;
mod dictionary;
mod dispositions;
mod docs;
#[cfg(feature = "dogfood")]
mod dogfood;
mod domains;
mod export;
mod gaps;
mod graph;
mod graph_reference;
mod health;
mod impact;
mod import;
mod materialize;
mod merge_jsonl;
mod native;
mod orphans;
mod prime;
mod proposals;
mod questions;
mod refs;
mod repo;
mod report;
mod requirements;
mod resolutions;
mod rules;
mod schema;
mod sdk;
mod skills;
mod sources;
mod stale;
mod swarm_backtrace;
mod synthesis_packets;
mod thread;
mod topics;
mod traceability;
mod updates;
mod validate;
mod wiki;

#[allow(clippy::redundant_pub_crate)]
pub(super) use export::{export_scope, ScopeExport};

#[allow(clippy::too_many_lines)]
#[allow(clippy::redundant_pub_crate)]
pub(super) async fn dispatch(command: Command, quiet: bool) -> anyhow::Result<()> {
    match command {
        Command::CargoInit {
            package,
            ste_onboarding,
            ste_pdf,
        } => {
            tokio::task::spawn_blocking(move || {
                cargo_init::handle(package.as_deref(), ste_onboarding, ste_pdf)
            })
            .await??;
        }
        Command::Init {
            path,
            scope,
            path_prefix,
            disposition_actor_id,
            clear_disposition_actors,
            ste_onboarding,
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
                        ste_onboarding,
                        ste_pdf,
                        invocation_channel,
                        package_manager,
                    },
                )
            })
            .await??;
        }
        Command::Check {
            repo, strict, base, ..
        } => {
            check::check(&repo, strict, base.as_deref())?;
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
        Command::Sources { command } => {
            sources::handle(command).await?;
        }
        Command::Requirements { command } => {
            requirements::handle(command).await?;
        }
        Command::GraphReference { command } => {
            graph_reference::handle(command)?;
        }
        Command::Domains { command } => {
            domains::handle(command).await?;
        }
        Command::Boundaries { command } => {
            boundaries::handle(command).await?;
        }
        Command::Topics { command } => {
            topics::handle(command).await?;
        }
        Command::Questions { command } => {
            questions::handle(command, quiet).await?;
        }
        Command::Graph {
            requirement_id,
            repo,
            scope,
            ..
        } => {
            graph::handle(requirement_id, repo, scope)?;
        }
        Command::Resolutions { command } => {
            resolutions::handle(command).await?;
        }
        Command::Rules { command } => {
            rules::handle(command).await?;
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
        Command::Thread { command } => {
            thread::handle(command)?;
        }
        Command::Contributions { command } => {
            contributions::handle(command, quiet)?;
        }
        Command::SynthesisPackets { command } => {
            synthesis_packets::handle(command, quiet)?;
        }
        Command::Proposals { command } => {
            proposals::handle(command, quiet)?;
        }
        Command::Dispositions { command } => {
            dispositions::handle(command)?;
        }
        Command::Prime {
            repo,
            scope,
            format,
            include_threads,
        } => {
            prime::handle(repo, scope, format, include_threads)?;
        }
        Command::Impact {
            id,
            repo,
            scope,
            node_type,
            max_hops,
            follow_indirect,
            ..
        } => {
            impact::handle(id, repo, scope, &node_type, max_hops, follow_indirect)?;
        }
        Command::Stale {
            base,
            head,
            since,
            repo,
            scope,
            strict,
            format,
        } => {
            stale::handle(&repo, scope, base, head, since, strict, format)?;
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
        Command::Sdk { command } => {
            sdk::handle(command).await?;
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
