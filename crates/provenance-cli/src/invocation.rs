use crate::{catalog_cli, cli::Cli, handlers};
use clap::{CommandFactory as _, Parser as _};
use provenance_cli::porcelain;
use provenance_core::protocol::{SearchQuery, QUERY_DEFAULT_LIMIT};
use provenance_core::{NodeType, SDK_PROTOCOL_VERSION};

pub(crate) mod grammar;
use grammar::{CatalogArgs, SearchArgs, TargetArgs};

#[cfg(test)]
mod tests;

/// One command selected from a declared grammar.
pub enum Invocation {
    Builtin(Cli),
    Catalog(catalog_cli::Invocation),
    Get(GetInvocation),
    Search(SearchInvocation),
    Target(TargetInvocation),
}

pub struct GlobalContext {
    pub repo: String,
    pub scope: String,
    pub quiet: bool,
}

pub struct GetInvocation {
    context: GlobalContext,
    format: Option<porcelain::OutputFormat>,
    input: provenance_porcelain::get::GetInput,
}

pub struct SearchInvocation {
    context: GlobalContext,
    format: Option<porcelain::OutputFormat>,
    query: SearchQuery,
}

pub struct TargetInvocation {
    context: GlobalContext,
    format: Option<porcelain::OutputFormat>,
    target: String,
    action: provenance_transport::porcelain::Action,
    kind: Option<NodeType>,
    matches: clap::ArgMatches,
}

impl Invocation {
    pub fn parse(arguments: Vec<String>) -> anyhow::Result<Self> {
        let Some(word) = grammar::command_word(&arguments) else {
            return Ok(Self::Builtin(
                Cli::try_parse_from(arguments).unwrap_or_else(|error| error.exit()),
            ));
        };
        if word == "search" {
            let args = SearchArgs::try_parse_from(arguments).unwrap_or_else(|error| error.exit());
            debug_assert_eq!(args.command, "search");
            return Ok(Self::Search(SearchInvocation {
                context: args.common.context(),
                format: args.common.format(),
                query: SearchQuery {
                    protocol_version: Some(SDK_PROTOCOL_VERSION),
                    cursor: args.cursor,
                    text: args.text,
                    node_types: args.kind,
                    limit: args.limit.unwrap_or(QUERY_DEFAULT_LIMIT),
                },
            }));
        }
        if Cli::command()
            .get_subcommands()
            .any(|command| command.get_name() == word)
        {
            return Ok(Self::Builtin(
                Cli::try_parse_from(arguments).unwrap_or_else(|error| error.exit()),
            ));
        }
        if catalog_cli::is_collection(word) {
            let command = catalog_cli::command(word)?;
            let matches = command
                .try_get_matches_from(arguments)
                .unwrap_or_else(|error| error.exit());
            let args = CatalogArgs::from_matches(&matches);
            return Ok(Self::Catalog(catalog_cli::Invocation::new(args, &matches)?));
        }

        let command = catalog_cli::target_command()?;
        let matches = command
            .try_get_matches_from(arguments)
            .unwrap_or_else(|error| error.exit());
        let args = TargetArgs::from_matches(&matches);
        let format = args.common.format();
        let context = args.common.context();
        if args.action.as_deref().is_none_or(|action| action == "get") {
            catalog_cli::ensure_only_fields(&matches, &["kind", "view", "depth", "limit"])?;
            let mut input = provenance_porcelain::get::GetInput::new(
                args.target,
                args.view.unwrap_or_default().into(),
            );
            input.max_depth = args.depth;
            input.returned_kinds = args.kind;
            input.limit = args.limit;
            return Ok(Self::Get(GetInvocation {
                context,
                format,
                input,
            }));
        }
        let action = args
            .action
            .as_deref()
            .and_then(provenance_transport::porcelain::Action::parse)
            .ok_or_else(|| anyhow::anyhow!("unsupported target action"))?;
        let kind = args
            .record_type
            .as_deref()
            .map(NodeType::parse)
            .transpose()
            .map_err(|_| anyhow::anyhow!("unsupported record type"))?;
        anyhow::ensure!(
            (action == provenance_transport::porcelain::Action::Create) == kind.is_some(),
            "create requires --type and existing-record actions infer it"
        );
        Ok(Self::Target(TargetInvocation {
            context,
            format,
            target: args.target,
            action,
            kind,
            matches,
        }))
    }

    pub async fn dispatch(self) -> anyhow::Result<()> {
        match self {
            Self::Builtin(cli) => handlers::dispatch(cli.command, cli.quiet).await,
            Self::Catalog(invocation) => catalog_cli::dispatch(invocation).await,
            Self::Get(invocation) => {
                porcelain::dispatch_get(
                    &invocation.context.repo,
                    &invocation.context.scope,
                    invocation.format,
                    invocation.input,
                )
                .await
            }
            Self::Search(invocation) => {
                porcelain::dispatch_search(
                    &invocation.context.repo,
                    &invocation.context.scope,
                    invocation.format,
                    invocation.query,
                )
                .await
            }
            Self::Target(invocation) => invocation.dispatch().await,
        }
    }
}

impl TargetInvocation {
    async fn dispatch(self) -> anyhow::Result<()> {
        catalog_cli::dispatch_target(
            self.context,
            self.format,
            self.target,
            self.action,
            self.kind,
            self.matches,
        )
        .await
    }
}
