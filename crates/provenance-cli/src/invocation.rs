pub use crate::registry_cli::root_command;
use crate::{catalog_cli, handlers};
use handlers::{GraphInput, RepositoryInput, ServerInput, WorkspaceInput};
use provenance_cli::porcelain;
use provenance_core::NodeType;
use provenance_macros::rule;
use provenance_porcelain::action::{validate_target, Action};
use provenance_porcelain::get::View;
use provenance_transport::repository::RepoContext;

mod api;
mod discussion;
pub mod grammar;
mod help_kind;
mod positional_tip;
mod removed;
mod search_text;
use grammar::{
    ApiArgs, Delivery, DiscussionsArgs, DiscussionsRoute, SearchArgs, TargetArgs, TargetVerb,
};
use provenance_cli::exposed;
use provenance_cli::servers::Server;
use provenance_porcelain::check::CheckInput;
use provenance_porcelain::registry::{self, PrimeInput};

#[cfg(test)]
mod surface_tests;
#[cfg(test)]
mod tests;

/// One command selected from a declared grammar, and whether the caller asked for JSON.
pub struct Invocation {
    selected: Selected,
    json: bool,
}

enum Selected {
    Check(CheckInput, crate::registry_cli::BoundContext),
    Prime(PrimeInput),
    Repository(RepositoryInput, bool),
    Graph(GraphInput),
    Workspace(WorkspaceInput),
    Server(ServerInput),
    #[cfg(feature = "dogfood")]
    Dogfood(handlers::DogfoodInput, bool),
    Api(ApiArgs),
    Get(GetInvocation),
    Search(SearchArgs),
    DiscussionRoot(DiscussionsArgs, Box<clap::ArgMatches>),
    DiscussionTarget(TargetArgs, Action, Box<clap::ArgMatches>),
    Target(TargetInvocation),
    /// A refusal of a command line that the parser accepted. It takes the
    /// refusal path of the command, so `--format json` prints the object.
    Refused(anyhow::Error),
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
        removed::refuse(&arguments)?;
        if grammar::command_word(&arguments) == Some("help") {
            return Self::parse(help_arguments(arguments));
        }
        let Some(word) = grammar::command_word(&arguments) else {
            return Ok(Self::parse_root(&arguments));
        };
        match exposed::find(&[word]) {
            Some(registry::Command::Search) => {
                let args = search_text::parse(&arguments);
                let json = args.common.format.is_some();
                return Ok(Self::new(json, Selected::Search(args)));
            }
            Some(registry::Command::Api) => {
                let matches =
                    crate::registry_cli::root_word_matches(registry::Command::Api, &arguments)
                        .unwrap_or_else(|error| error.exit());
                let args = ApiArgs::from_matches(&matches);
                let json = args.common.format.is_some();
                return Ok(Self::new(json, Selected::Api(args)));
            }
            Some(
                registry::Command::Check
                | registry::Command::Prime
                | registry::Command::Repository(_)
                | registry::Command::Graph(_)
                | registry::Command::Workspace(_)
                | registry::Command::Dogfood(_),
            ) => {
                return Ok(Self::parse_root(&arguments));
            }
            Some(
                registry::Command::Get
                | registry::Command::Trace
                | registry::Command::Change
                | registry::Command::Action(_),
            )
            | None => {}
        }
        if word == "discussions" {
            let target_command = catalog_cli::target_command()?;
            if grammar::discussions_route(&arguments, &target_command)
                == DiscussionsRoute::Addressed
            {
                let matches = grammar::discussions_command()?
                    .try_get_matches_from(arguments)
                    .unwrap_or_else(|error| error.exit());
                let args = DiscussionsArgs::from_matches(&matches);
                let json = args.common.format.is_some();
                return Ok(Self::new(
                    json,
                    Selected::DiscussionRoot(args, Box::new(matches)),
                ));
            }
        }
        if root_command()
            .get_subcommands()
            .any(|command| command.get_name() == word)
        {
            return Ok(Self::parse_root(&arguments));
        }
        if is_collection(word) {
            refuse_collection_word(word);
        }

        Self::parse_target(arguments)
    }

    /// Parses a root command: a built-in command, or a registry command that the root
    /// parser shows with the built-in commands.
    fn parse_root(arguments: &[String]) -> Self {
        let matches = root_command()
            .try_get_matches_from(arguments)
            .unwrap_or_else(|error| positional_tip::with_tip(error, arguments).exit());
        let (words, selected) = crate::registry_cli::selected(&matches);
        let json = crate::registry_cli::asks_for_json(selected);
        let command = Server::find(&words).map_or_else(
            || Self::registry_selected(&matches, &words, selected),
            |server| ServerInput::from_matches(server, selected).map(Selected::Server),
        );
        let refused = |error| Selected::Refused(catalog_cli::refusal::usage(error));
        Self::new(json, command.unwrap_or_else(refused))
    }

    /// The registry command of a root line that the parser accepted.
    fn registry_selected(
        matches: &clap::ArgMatches,
        words: &[&str],
        selected: &clap::ArgMatches,
    ) -> anyhow::Result<Selected> {
        match exposed::find(words) {
            Some(registry::Command::Check) => {
                crate::registry_cli::input(&registry::Command::Check.declaration(), selected).map(
                    |input| Selected::Check(input, crate::registry_cli::bound_context(selected)),
                )
            }
            Some(registry::Command::Prime) => {
                crate::registry_cli::input(&registry::Command::Prime.declaration(), selected)
                    .map(Selected::Prime)
            }
            Some(registry::Command::Repository(command)) => {
                RepositoryInput::from_matches(command, selected)
                    .map(|input| Selected::Repository(input, matches.get_flag("quiet")))
            }
            Some(registry::Command::Graph(command)) => {
                GraphInput::from_matches(command, selected).map(Selected::Graph)
            }
            Some(registry::Command::Workspace(command)) => {
                WorkspaceInput::from_matches(command, selected).map(Selected::Workspace)
            }
            #[cfg(feature = "dogfood")]
            Some(registry::Command::Dogfood(command)) => {
                handlers::DogfoodInput::from_matches(command, selected)
                    .map(|input| Selected::Dogfood(input, matches.get_flag("quiet")))
            }
            _ => root_command()
                .error(
                    clap::error::ErrorKind::InvalidSubcommand,
                    format!("'{}' is not a command", words.join(" ")),
                )
                .exit(),
        }
    }

    /// The matches of the target parser. A help request prints the help of the selected
    /// action and kind and exits before validation.
    #[provenance_macros::rule("rule_record_help_exits_before_validation")]
    fn target_matches(arguments: Vec<String>) -> anyhow::Result<clap::ArgMatches> {
        let command = catalog_cli::target_command()?;
        let help_command = command.clone().mut_arg("type", |arg| {
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
                    .get_one::<String>("type")
                    .and_then(|kind| NodeType::parse(kind).ok())
                    .or_else(|| help_kind::resolve_record_kind(&selection, action));
                catalog_cli::print_target_help(command, action, kind)?;
                std::process::exit(0);
            }
            Err(error) => error.exit(),
        };
        Ok(matches)
    }

    /// Parses the target before its action and defaults to get.
    #[rule("rule_porcelain_cli_target_action_order")]
    fn parse_target(arguments: Vec<String>) -> anyhow::Result<Self> {
        let matches = Self::target_matches(arguments)?;
        let args = TargetArgs::from_matches(&matches);
        let json = args.common.format.is_some();
        let selected = match args.action {
            Some(TargetVerb::Action(action)) if Action::DISCUSSION.contains(&action) => {
                Ok(Selected::DiscussionTarget(args, action, Box::new(matches)))
            }
            _ => Self::target_selected(args, matches),
        };
        Ok(Self::new(json, selected.unwrap_or_else(Selected::Refused)))
    }

    /// The command of a target line that the parser accepted, or the refusal of
    /// an option or a value that the action does not take.
    fn target_selected(args: TargetArgs, matches: clap::ArgMatches) -> anyhow::Result<Selected> {
        let format = args.common.format();
        let context = args.common.context();
        if args.action == Some(TargetVerb::Trace) {
            catalog_cli::ensure_only_fields(&matches, &["kind", registry::DEPTH.name, "limit"])?;
            let input = provenance_porcelain::get::TraceInput {
                target: args.target,
                depth: args.depth,
                kind: args.kind,
                limit: args.limit,
            };
            return Ok(Selected::Get(GetInvocation {
                context,
                format,
                input: input.into(),
                delivery: Delivery::Read,
            }));
        }
        if matches!(args.action, None | Some(TargetVerb::Get)) {
            catalog_cli::ensure_only_fields(
                &matches,
                &[
                    "kind",
                    registry::VIEW.name,
                    registry::DEPTH.name,
                    "limit",
                    "review-link",
                    "no-open",
                    "follow",
                    "notify",
                    "thread",
                ],
            )?;
            let mut input = provenance_porcelain::get::GetInput::new(
                args.target,
                args.view
                    .as_deref()
                    .and_then(View::parse)
                    .unwrap_or_default(),
            );
            input.depth = args.depth;
            input.kind = args.kind;
            input.limit = args.limit;
            (input.notify, input.thread) = notify(&matches)?;
            return Ok(Selected::Get(GetInvocation {
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
            .map_err(|error| catalog_cli::action_usage(&error))?;
        Ok(Selected::Target(TargetInvocation {
            context,
            format,
            target: args.target,
            action,
            kind,
            matches,
        }))
    }

    const fn new(json: bool, selected: Selected) -> Self {
        Self { selected, json }
    }

    /// Whether the caller gave `--format json`, so a refusal prints as one
    /// `{error,meta}` object instead of text.
    pub const fn json_output(&self) -> bool {
        self.json
    }

    pub async fn dispatch(self) -> anyhow::Result<()> {
        match self.selected {
            Selected::Check(input, context) => handlers::check_command(input, context).await,
            Selected::Prime(input) => handlers::prime_command(&input),
            Selected::Repository(input, quiet) => handlers::repository_command(input, quiet).await,
            Selected::Graph(input) => handlers::graph_command(input).await,
            Selected::Workspace(input) => handlers::workspace_command(input).await,
            Selected::Server(input) => handlers::server_command(input).await,
            #[cfg(feature = "dogfood")]
            Selected::Dogfood(input, quiet) => handlers::dogfood_command(input, quiet).await,
            Selected::Api(args) => api::dispatch(args).await,
            Selected::Get(invocation) => {
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
                        &provenance_cli::porcelain::place(&invocation.context.repo),
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
            Selected::Search(args) => args.dispatch().await,
            Selected::DiscussionRoot(args, matches) => {
                discussion::dispatch_root(args, &matches).await
            }
            Selected::DiscussionTarget(args, action, matches) => {
                discussion::dispatch_target(args, action, &matches).await
            }
            Selected::Target(invocation) => invocation.dispatch().await,
            Selected::Refused(error) => Err(error),
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

/// `provenance help [words...]` as `provenance [words...] --help`, so both print the
/// same help.
#[rule("rule_cli_help_command_shows_help")]
fn help_arguments(arguments: Vec<String>) -> Vec<String> {
    let mut removed = false;
    let mut arguments = arguments
        .into_iter()
        .filter(|argument| {
            let help = !removed && argument == "help";
            removed |= help;
            !help
        })
        .collect::<Vec<_>>();
    arguments.push("--help".to_owned());
    arguments
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
fn notify(
    matches: &clap::ArgMatches,
) -> anyhow::Result<(
    Option<provenance_porcelain::get::NotifyClient>,
    Option<String>,
)> {
    use provenance_porcelain::get::NotifyClient;
    let Some(client) = matches.get_one::<String>("notify") else {
        return Ok((None, None));
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
        return Err(provenance_cli::refusal::CommandRefusal::missing(
            "thread",
            "--notify needs --thread or the CODEX_THREAD_ID environment variable",
        )
        .into());
    };
    Ok((Some(client), Some(thread)))
}
