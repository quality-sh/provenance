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
mod domains;
mod edges;
mod export;
mod gaps;
mod graph;
mod graph_reference;
mod health;
mod impact;
mod import;
mod materialize;
mod merge_jsonl;
mod orphans;
mod prime;
mod proposals;
mod questions;
mod repo;
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
mod validate;
mod wiki;

#[allow(clippy::redundant_pub_crate)]
pub(super) use export::{export_scope, ScopeExport};

/// The parsed actor claim from the global `--actor-id` flag, threaded
/// top-down into every mutating handler. Read-only commands ignore it.
#[allow(clippy::too_many_lines)]
#[allow(clippy::redundant_pub_crate)]
pub(super) async fn dispatch(
    command: Command,
    quiet: bool,
    actor_claim: Option<&provenance_core::RbacClaim>,
) -> anyhow::Result<()> {
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
            // The claim is cloned before the blocking closure so the closure
            // stays 'static; the borrow itself never crosses the boundary.
            let actor_claim = actor_claim.cloned();
            tokio::task::spawn_blocking(move || {
                repo::init(
                    &path,
                    repo::InitOptions {
                        scope,
                        path_prefix,
                        disposition_actor_ids: disposition_actor_id,
                        clear_disposition_actors,
                        actor_claim,
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
            repo,
            strict,
            base,
            format,
        } => {
            check::check(&repo, strict, base.as_deref(), format)?;
        }
        Command::Docs { command } => {
            docs::handle(command).await?;
        }
        Command::Dictionary { command } => {
            dictionary::handle(command, actor_claim)?;
        }
        Command::Wiki { command } => {
            wiki::handle(command).await?;
        }
        Command::Materialize { repo, format } => {
            materialize::handle(repo, format).await?;
        }
        Command::Sources { command } => {
            sources::handle(command, actor_claim)?;
        }
        Command::Requirements { command } => {
            requirements::handle(command, actor_claim)?;
        }
        Command::Edges { command } => {
            edges::handle(command, actor_claim)?;
        }
        Command::GraphReference { command } => {
            graph_reference::handle(command)?;
        }
        Command::Domains { command } => {
            domains::handle(command, actor_claim)?;
        }
        Command::Boundaries { command } => {
            boundaries::handle(command, actor_claim)?;
        }
        Command::Topics { command } => {
            topics::handle(command, actor_claim)?;
        }
        Command::Questions { command } => {
            questions::handle(command, quiet, actor_claim)?;
        }
        Command::Graph {
            requirement_id,
            repo,
            scope,
            format,
        } => {
            graph::handle(requirement_id, repo, scope, format)?;
        }
        Command::Resolutions { command } => {
            resolutions::handle(command, actor_claim)?;
        }
        Command::Rules { command } => {
            rules::handle(command, actor_claim)?;
        }
        Command::Traceability {
            rule_id,
            repo,
            scope,
            format,
        } => {
            traceability::handle(rule_id, repo, scope, format)?;
        }
        Command::Gaps {
            repo,
            scope,
            format,
        } => {
            gaps::handle(repo, scope, format)?;
        }
        Command::Thread { command } => {
            thread::handle(command, actor_claim)?;
        }
        Command::Contributions { command } => {
            contributions::handle(command, quiet, actor_claim)?;
        }
        Command::SynthesisPackets { command } => {
            synthesis_packets::handle(command, quiet, actor_claim)?;
        }
        Command::Proposals { command } => {
            proposals::handle(command, quiet, actor_claim)?;
        }
        Command::Dispositions { command } => {
            dispositions::handle(command, actor_claim)?;
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
            format,
        } => {
            impact::handle(
                id,
                repo,
                scope,
                &node_type,
                max_hops,
                follow_indirect,
                format,
            )?;
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
        Command::Health {
            repo,
            scope,
            format,
        } => {
            health::handle(repo, scope, format)?;
        }
        Command::Orphans {
            repo,
            scope,
            format,
        } => {
            orphans::handle(repo, scope, format)?;
        }
        Command::Coverage { command } => {
            coverage::handle(command)?;
        }
        Command::Sdk { command } => {
            sdk::handle(command)?;
        }
        Command::SwarmBacktrace { command } => {
            swarm_backtrace::handle(command, actor_claim)?;
        }
        Command::Skills { command } => {
            skills::handle(command)?;
        }
        Command::Schema { command } => {
            schema::handle(command)?;
        }
        Command::Validate {
            artifact,
            input,
            format,
        } => {
            validate::handle(artifact, &input, format)?;
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
            format,
        } => {
            import::handle(repo, scope, input, dry_run, format, actor_claim)?;
        }
        Command::MergeJsonl {
            base,
            ours,
            theirs,
            output,
            path,
            actor_id,
            format,
        } => {
            merge_jsonl::handle(
                &base,
                &ours,
                &theirs,
                output,
                path.as_deref(),
                actor_id,
                format,
            )?;
        }
    }
    Ok(())
}
