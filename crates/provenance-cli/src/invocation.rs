use crate::{catalog_cli, cli::Cli, handlers};
use clap::{CommandFactory as _, FromArgMatches as _, Parser as _};
use provenance_cli::porcelain;
use provenance_cli::repo_context::RepoContext;
use provenance_core::protocol::{SearchQuery, QUERY_DEFAULT_LIMIT};
use provenance_core::{NodeType, SDK_PROTOCOL_VERSION};
use provenance_porcelain::action::{validate_target, Action};
use provenance_porcelain::get::View;

mod api;
mod discussion;
pub mod grammar;
use grammar::{
    ApiArgs, ApiCommand, CatalogArgs, Delivery, DiscussionsArgs, DiscussionsCommand,
    DiscussionsRoute, RootWord, SearchArgs, SearchCommand, TargetArgs, TargetVerb,
};

#[cfg(test)]
mod tests;

/// One command selected from a declared grammar.
pub enum Invocation {
    Builtin(Cli),
    Api(ApiArgs),
    Catalog(catalog_cli::Invocation),
    Get(GetInvocation),
    Search(SearchArgs),
    DiscussionRoot(DiscussionsArgs, Box<clap::ArgMatches>),
    DiscussionTarget(TargetArgs, Action, Box<clap::ArgMatches>),
    Target(TargetInvocation),
}

pub struct GlobalContext {
    pub repo: RepoContext,
    pub quiet: bool,
}

pub struct GetInvocation {
    context: GlobalContext,
    format: Option<porcelain::OutputFormat>,
    input: provenance_porcelain::get::GetInput,
    delivery: Delivery,
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
        match RootWord::parse(word) {
            Some(RootWord::Search) => {
                let args =
                    SearchCommand::try_parse_from(arguments).unwrap_or_else(|error| error.exit());
                debug_assert_eq!(args.command, "search");
                return Ok(Self::Search(args.args));
            }
            Some(RootWord::Api) => {
                let command =
                    ApiCommand::try_parse_from(arguments).unwrap_or_else(|error| error.exit());
                debug_assert_eq!(command.command, "api");
                return Ok(Self::Api(command.args));
            }
            None => {}
        }
        if word == "discussions" {
            let target_command = catalog_cli::target_command()?;
            if grammar::discussions_route(&arguments, &target_command)
                == DiscussionsRoute::Addressed
            {
                let matches = grammar::discussions_command()?
                    .try_get_matches_from(arguments)
                    .unwrap_or_else(|error| error.exit());
                let command = DiscussionsCommand::from_arg_matches(&matches)
                    .unwrap_or_else(|error| error.exit());
                debug_assert_eq!(command.command, "discussions");
                return Ok(Self::DiscussionRoot(command.args, Box::new(matches)));
            }
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

        Self::parse_target(arguments)
    }

    fn parse_target(arguments: Vec<String>) -> anyhow::Result<Self> {
        let command = catalog_cli::target_command()?;
        let matches = match command.clone().try_get_matches_from(arguments.clone()) {
            Ok(matches) => matches,
            Err(error) if error.kind() == clap::error::ErrorKind::DisplayHelp => {
                let selection = command.clone().disable_help_flag(true).ignore_errors(true)
                    .get_matches_from(arguments.into_iter().filter(|arg| arg != "--help" && arg != "-h"));
                catalog_cli::print_target_help(
                    command,
                    selection.get_one::<TargetVerb>("action").copied(),
                    selection.get_one::<NodeType>("record_type").copied(),
                )?;
                std::process::exit(0);
            }
            Err(error) => error.exit(),
        };
        let args = TargetArgs::from_matches(&matches);
        if let Some(TargetVerb::Action(action)) = args.action {
            if Action::DISCUSSION.contains(&action) {
                return Ok(Self::DiscussionTarget(args, action, Box::new(matches)));
            }
        }
        let format = args.common.format();
        let context = args.common.context();
        if matches!(args.action, None | Some(TargetVerb::Get)) {
            catalog_cli::ensure_only_fields(
                &matches,
                &[
                    "kind",
                    "view",
                    "depth",
                    "limit",
                    "review-link",
                    "no-open",
                    "follow",
                ],
            );
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
                delivery: args.delivery,
            }));
        }
        let Some(TargetVerb::Action(action)) = args.action else {
            anyhow::bail!("unsupported target action");
        };
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
            Self::Api(args) => api::dispatch(args).await,
            Self::Catalog(invocation) => catalog_cli::dispatch(invocation).await,
            Self::Get(invocation) => {
                if invocation.delivery == Delivery::Follow {
                    anyhow::ensure!(
                        invocation.input.view == View::Review
                            && invocation.format == Some(porcelain::OutputFormat::Json),
                        "--follow requires --view review --format json"
                    );
                    return crate::review::follow::run(&invocation.context.repo, invocation.input)
                        .await;
                }
                if let Delivery::ReviewLink { open } = invocation.delivery {
                    return crate::review_link::print(
                        &invocation.context.repo,
                        &invocation.input.target,
                        invocation.format,
                        !open,
                    )
                    .await;
                }
                porcelain::dispatch_get(
                    invocation.context.repo.repo.as_str(),
                    &invocation.context.repo.scope,
                    invocation.format,
                    invocation.input,
                )
                .await
            }
            Self::Search(args) => args.dispatch().await,
            Self::DiscussionRoot(args, matches) => discussion::dispatch_root(args, &matches).await,
            Self::DiscussionTarget(args, action, matches) => {
                discussion::dispatch_target(args, action, &matches).await
            }
            Self::Target(invocation) => invocation.dispatch().await,
        }
    }
}

impl SearchArgs {
    pub(crate) async fn dispatch(self) -> anyhow::Result<()> {
        let parameters = provenance_store::operations::catalog::definitions()
            .iter()
            .find_map(|definition| {
                definition
                    .registration
                    .queries
                    .iter()
                    .find(|query| query.name == "search")
                    .map(|query| query.parameters.as_slice())
            })
            .ok_or_else(|| anyhow::anyhow!("the operation catalog does not declare root search"))?;
        let default_exclude_terminal = crate::read_policy::default_exclude_terminal(parameters)
            .ok_or_else(|| {
                anyhow::anyhow!("the root search query does not declare exclude_terminal")
            })?;
        let query = SearchQuery {
            protocol_version: Some(SDK_PROTOCOL_VERSION),
            cursor: self.cursor,
            exclude_terminal: if self.include_terminal {
                crate::read_policy::exclude_terminal(true)
            } else {
                default_exclude_terminal
            },
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
