use crate::{catalog_cli, cli::Cli, handlers};
use clap::CommandFactory as _;
use handlers::{GraphInput, RepositoryInput, WorkspaceInput};
use provenance_cli::porcelain;
use provenance_cli::repo_context::RepoContext;
use provenance_core::NodeType;
use provenance_macros::rule;
use provenance_porcelain::action::{validate_target, Action};
use provenance_porcelain::get::View;
use provenance_porcelain::search::SearchInput;

mod api;
mod discussion;
pub mod grammar;
mod help_kind;
use grammar::{
    ApiArgs, Delivery, DiscussionsArgs, DiscussionsRoute, SearchArgs, TargetArgs, TargetVerb,
};
use provenance_porcelain::registry::{self, CheckCliInput, Interface, PrimeInput};

#[cfg(test)]
mod tests;

/// One command selected from a declared grammar.
pub enum Invocation {
    Check(CheckCliInput),
    Prime(PrimeInput),
    Repository(RepositoryInput, bool),
    Graph(GraphInput),
    Workspace(WorkspaceInput),
    #[cfg(feature = "dogfood")]
    Dogfood(handlers::DogfoodInput, bool),
    Api(ApiArgs),
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
    /// Uses get for an omitted action after excluding reserved command words.
    #[rule("rule_porcelain_get_is_default_action")]
    pub fn parse(arguments: Vec<String>) -> anyhow::Result<Self> {
        let Some(word) = grammar::command_word(&arguments) else {
            return Ok(Self::parse_root(arguments));
        };
        match registry::Command::find(Interface::Cli, &[word]) {
            Some(registry::Command::Search) => {
                let matches = crate::registry_cli::root_word_command(registry::Command::Search)
                    .try_get_matches_from(arguments)
                    .unwrap_or_else(|error| error.exit());
                return Ok(Self::Search(SearchArgs::from_matches(&matches)));
            }
            Some(registry::Command::Api) => {
                let matches = crate::registry_cli::root_word_command(registry::Command::Api)
                    .try_get_matches_from(arguments)
                    .unwrap_or_else(|error| error.exit());
                return Ok(Self::Api(ApiArgs::from_matches(&matches)));
            }
            Some(
                registry::Command::Check
                | registry::Command::Prime
                | registry::Command::Repository(_)
                | registry::Command::Graph(_)
                | registry::Command::Workspace(_)
                | registry::Command::Dogfood(_),
            ) => {
                return Ok(Self::parse_root(arguments));
            }
            Some(registry::Command::Get | registry::Command::Action(_)) | None => {}
        }
        if word == "discussions" {
            let target_command = catalog_cli::target_command()?;
            if grammar::discussions_route(&arguments, &target_command)
                == DiscussionsRoute::Addressed
            {
                let matches = grammar::discussions_command()?
                    .try_get_matches_from(arguments)
                    .unwrap_or_else(|error| error.exit());
                return Ok(Self::DiscussionRoot(
                    DiscussionsArgs::from_matches(&matches),
                    Box::new(matches),
                ));
            }
        }
        if root_command()
            .get_subcommands()
            .any(|command| command.get_name() == word)
        {
            return Ok(Self::parse_root(arguments));
        }
        if is_collection(word) {
            refuse_collection_word(word);
        }

        Self::parse_target(arguments)
    }

    /// Parses a root command: a built-in command, or a registry command that the root
    /// parser shows with the built-in commands.
    fn parse_root(arguments: Vec<String>) -> Self {
        let matches = root_command()
            .try_get_matches_from(arguments)
            .unwrap_or_else(|error| error.exit());
        let (words, selected) = crate::registry_cli::selected(&matches);
        match registry::Command::find(Interface::Cli, &words) {
            Some(registry::Command::Check) => Self::Check(
                crate::registry_cli::input(&registry::Command::Check.declaration(), selected)
                    .unwrap_or_else(|error| catalog_cli::usage_error(error)),
            ),
            Some(registry::Command::Prime) => Self::Prime(
                crate::registry_cli::input(&registry::Command::Prime.declaration(), selected)
                    .unwrap_or_else(|error| catalog_cli::usage_error(error)),
            ),
            Some(registry::Command::Repository(command)) => Self::Repository(
                RepositoryInput::from_matches(command, selected)
                    .unwrap_or_else(|error| catalog_cli::usage_error(error)),
                matches.get_flag("quiet"),
            ),
            Some(registry::Command::Graph(command)) => Self::Graph(
                GraphInput::from_matches(command, selected)
                    .unwrap_or_else(|error| catalog_cli::usage_error(error)),
            ),
            Some(registry::Command::Workspace(command)) => Self::Workspace(
                WorkspaceInput::from_matches(command, selected)
                    .unwrap_or_else(|error| catalog_cli::usage_error(error)),
            ),
            #[cfg(feature = "dogfood")]
            Some(registry::Command::Dogfood(command)) => Self::Dogfood(
                handlers::DogfoodInput::from_matches(command, selected)
                    .unwrap_or_else(|error| catalog_cli::usage_error(error)),
                matches.get_flag("quiet"),
            ),
            _ => root_command()
                .error(
                    clap::error::ErrorKind::InvalidSubcommand,
                    format!("'{}' is not a command", words.join(" ")),
                )
                .exit(),
        }
    }

    /// Parses the target before its action, defaults to get, and handles help before validation.
    #[provenance_macros::rule("rule_record_help_exits_before_validation")]
    #[rule("rule_porcelain_cli_target_action_order")]
    fn parse_target(arguments: Vec<String>) -> anyhow::Result<Self> {
        let command = catalog_cli::target_command()?;
        let help_command = command.clone().mut_arg("record_type", |arg| {
            arg.value_parser(clap::builder::StringValueParser::new())
        });
        let matches = match help_command.clone().try_get_matches_from(arguments.clone()) {
            Ok(_) => command
                .try_get_matches_from(arguments)
                .unwrap_or_else(|error| error.exit()),
            Err(error) if error.kind() == clap::error::ErrorKind::DisplayHelp => {
                let selection = help_command
                    .disable_help_flag(true)
                    .ignore_errors(true)
                    .get_matches_from(
                        arguments
                            .into_iter()
                            .filter(|arg| arg != "--help" && arg != "-h"),
                    );
                let action = selection.get_one::<TargetVerb>("action").copied();
                let kind = selection
                    .get_one::<String>("record_type")
                    .and_then(|kind| NodeType::parse(kind).ok())
                    .or_else(|| help_kind::resolve_record_kind(&selection, action));
                catalog_cli::print_target_help(command, action, kind)?;
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
                    "notify",
                    "thread",
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
            input.notify = notify(&matches)?;
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

    /// Whether the caller asked for JSON, so a refusal prints as one
    /// `{error,meta}` object instead of text.
    pub fn json_output(&self) -> bool {
        let format = match self {
            Self::Check(_)
            | Self::Prime(_)
            | Self::Repository(..)
            | Self::Graph(_)
            | Self::Workspace(_) => None,
            #[cfg(feature = "dogfood")]
            Self::Dogfood(..) => None,
            Self::Api(args) => args.common.format(),
            Self::Get(invocation) => invocation.format,
            Self::Search(args) => args.common.format(),
            Self::DiscussionRoot(args, _) => args.common.format(),
            Self::DiscussionTarget(args, _, _) => args.common.format(),
            Self::Target(invocation) => invocation.format,
        };
        format == Some(porcelain::OutputFormat::Json)
    }

    pub async fn dispatch(self) -> anyhow::Result<()> {
        match self {
            Self::Check(input) => handlers::check_command(input).await,
            Self::Prime(input) => handlers::prime_command(&input),
            Self::Repository(input, quiet) => handlers::repository_command(input, quiet).await,
            Self::Graph(input) => handlers::graph_command(input),
            Self::Workspace(input) => handlers::workspace_command(input).await,
            #[cfg(feature = "dogfood")]
            Self::Dogfood(input, quiet) => handlers::dogfood_command(input, quiet),
            Self::Api(args) => api::dispatch(args).await,
            Self::Get(invocation) => {
                if invocation.delivery == Delivery::Follow {
                    anyhow::ensure!(
                        invocation.input.view == View::Review
                            && invocation.format == Some(porcelain::OutputFormat::Json),
                        "--follow requires --view review --format json"
                    );
                    // The Ctrl-C handler is set up when the follower first polls this, after it
                    // connects; until then Ctrl-C ends the process at once.
                    let stop = async {
                        anyhow::Context::context(
                            tokio::signal::ctrl_c().await,
                            "cannot install the Ctrl-C handler",
                        )
                    };
                    return provenance_review::follow(
                        &invocation.context.repo.place(),
                        invocation.input,
                        tokio::io::stdout(),
                        stop,
                    )
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
        let parameters = provenance_contract::definitions()
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
        // A file search returns every bound Rule, so only a text search takes the default.
        let exclude_terminal = if self.include_terminal {
            Some(crate::read_policy::exclude_terminal(true))
        } else {
            self.file.is_none().then_some(default_exclude_terminal)
        };
        let query = SearchInput {
            text: self.text,
            node_types: self.kind,
            exclude_terminal,
            cursor: self.cursor,
            limit: self.limit,
            file: self.file,
            symbol: self.symbol,
            line: self.line,
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

/// The root parser: the built-in commands and the word commands that it lists.
pub fn root_command() -> clap::Command {
    [
        registry::Command::Search,
        registry::Command::Check,
        registry::Command::Prime,
    ]
    .into_iter()
    .chain(registry::Command::ALL.iter().copied().filter(|command| {
        matches!(
            command,
            registry::Command::Repository(_)
                | registry::Command::Graph(_)
                | registry::Command::Workspace(_)
                | registry::Command::Dogfood(_)
        ) && command.is_present(registry::Interface::Cli)
    }))
    .fold(Cli::command(), crate::registry_cli::add_subcommand)
}

/// Whether a word names an API collection, the first segment of a catalog path.
fn is_collection(word: &str) -> bool {
    provenance_contract::definitions()
        .iter()
        .any(|definition| definition.path.split('/').nth(1) == Some(word))
}

/// Refuses a command that starts with an API collection name and shows the
/// help command. Plumbing operations are reachable only through `api`.
#[rule("rule_cli_collection_word_shows_help")]
fn refuse_collection_word(word: &str) -> ! {
    clap::Error::raw(
        clap::error::ErrorKind::InvalidSubcommand,
        format!(
            "'{word}' is an API collection, not a command\n\n\
             For more information, try 'provenance --help'.\n"
        ),
    )
    .exit()
}

/// Reads `--notify` and its thread: `--thread`, else `CODEX_THREAD_ID` from a Codex session.
fn notify(matches: &clap::ArgMatches) -> anyhow::Result<Option<provenance_porcelain::get::Notify>> {
    use provenance_porcelain::get::{Notify, NotifyClient};
    let Some(client) = matches.get_one::<String>("notify") else {
        return Ok(None);
    };
    let client = if client == "off" {
        NotifyClient::Off
    } else {
        NotifyClient::Codex
    };
    let thread = matches
        .get_one::<String>("thread")
        .cloned()
        .or_else(|| std::env::var("CODEX_THREAD_ID").ok())
        .filter(|thread| !thread.is_empty());
    let Some(thread) = thread else {
        anyhow::bail!("--notify needs --thread or the CODEX_THREAD_ID environment variable");
    };
    Ok(Some(Notify { client, thread }))
}
