use crate::{catalog_cli, cli::Cli, handlers};
use clap::{CommandFactory as _, Parser as _};
use provenance_cli::porcelain;
use provenance_core::protocol::{SearchQuery, QUERY_DEFAULT_LIMIT};
use provenance_core::{NodeType, SDK_PROTOCOL_VERSION};
use provenance_porcelain::action::{validate_target, Action};
use provenance_porcelain::discussion::DiscussionAction;
use provenance_porcelain::get::View;

mod discussion;
pub mod grammar;
use grammar::{
    CatalogArgs, DiscussionsArgs, DiscussionsCommand, SearchArgs, SearchCommand, TargetArgs,
};

#[cfg(test)]
mod tests;

/// One command selected from a declared grammar.
pub(crate) enum Invocation {
    Builtin(Cli),
    Catalog(catalog_cli::Invocation),
    Get(GetInvocation),
    Search(SearchArgs),
    DiscussionRoot(DiscussionsArgs),
    DiscussionTarget(TargetArgs, DiscussionAction, clap::ArgMatches),
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

pub struct TargetInvocation {
    context: GlobalContext,
    format: Option<porcelain::OutputFormat>,
    target: String,
    action: Action,
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
            let args =
                SearchCommand::try_parse_from(arguments).unwrap_or_else(|error| error.exit());
            debug_assert_eq!(args.command, "search");
            return Ok(Self::Search(args.args));
        }
        if word == "discussions" {
            let command =
                DiscussionsCommand::try_parse_from(arguments).unwrap_or_else(|error| error.exit());
            debug_assert_eq!(command.command, "discussions");
            let args = command.args;
            if args.discussion_id.as_deref() == Some("get") && args.action.is_none() {
                if args.status.is_some() || args.limit.is_some() || args.cursor.is_some() {
                    catalog_cli::usage_error("graph get does not accept Discussion list options");
                }
                return Ok(Self::Get(GetInvocation {
                    context: args.common.context(),
                    format: args.common.format(),
                    input: provenance_porcelain::get::GetInput::new("discussions", View::Record),
                }));
            }
            return Ok(Self::DiscussionRoot(args));
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
            return Ok(Self::Catalog(catalog_cli::Invocation::new(&args, &matches)));
        }

        let command = catalog_cli::target_command()?;
        let matches = command
            .try_get_matches_from(arguments)
            .unwrap_or_else(|error| error.exit());
        let args = TargetArgs::from_matches(&matches);
        if let Some(action) = args.action.as_deref().and_then(DiscussionAction::parse) {
            return Ok(Self::DiscussionTarget(args, action, matches));
        }
        let format = args.common.format();
        let context = args.common.context();
        if args.action.as_deref().is_none_or(|action| action == "get") {
            catalog_cli::ensure_only_fields(&matches, &["kind", "view", "depth", "limit"]);
            let mut input = provenance_porcelain::get::GetInput::new(
                args.target,
                args.view
                    .as_deref()
                    .and_then(View::parse)
                    .unwrap_or_default(),
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
            .and_then(Action::parse)
            .ok_or_else(|| anyhow::anyhow!("unsupported target action"))?;
        let kind = args.record_type;
        validate_target(action, &args.target, kind)
            .unwrap_or_else(|error| catalog_cli::usage_error(error));
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
            Self::Search(args) => args.dispatch().await,
            Self::DiscussionRoot(args) => discussion::dispatch_root(args).await,
            Self::DiscussionTarget(args, action, matches) => {
                discussion::dispatch_target(args, action, &matches).await
            }
            Self::Target(invocation) => invocation.dispatch().await,
        }
    }
}

impl SearchArgs {
    pub(crate) async fn dispatch(self) -> anyhow::Result<()> {
        let query = SearchQuery {
            protocol_version: Some(SDK_PROTOCOL_VERSION),
            cursor: self.cursor,
            text: self.text,
            node_types: self.kind,
            limit: self.limit.unwrap_or(QUERY_DEFAULT_LIMIT),
        };
        porcelain::dispatch_search(
            &self.common.repo,
            &self.common.scope,
            self.common.format(),
            query,
        )
        .await
    }
}

impl DiscussionsArgs {
    pub(crate) async fn dispatch(self) -> anyhow::Result<()> {
        discussion::dispatch_root(self).await
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
