use super::GlobalContext;
use crate::output::JsonFormat;
use clap::{
    builder::{PossibleValuesParser, TypedValueParser},
    ArgAction, ArgMatches, Args, Command, CommandFactory, FromArgMatches, Parser,
};
use provenance_cli::porcelain;
use provenance_core::NodeType;
use provenance_porcelain::action::Action;
use provenance_porcelain::get::View;

// Shared options for the Porcelain and catalog grammars.
#[derive(Args)]
#[group(skip)]
pub struct Common {
    #[arg(long, global = true, default_value = ".", allow_hyphen_values = true)]
    pub repo: String,
    #[arg(
        long,
        global = true,
        default_value = "default",
        allow_hyphen_values = true
    )]
    pub scope: String,
    #[arg(long, global = true, value_enum)]
    pub format: Option<JsonFormat>,
    #[arg(long, global = true)]
    pub quiet: bool,
}

impl Common {
    pub fn context(&self) -> GlobalContext {
        GlobalContext {
            repo: self.repo.clone(),
            scope: self.scope.clone(),
            quiet: self.quiet,
        }
    }

    pub fn format(&self) -> Option<porcelain::OutputFormat> {
        self.format.map(|_| porcelain::OutputFormat::Json)
    }
}

#[derive(Args)]
pub struct SearchArgs {
    #[command(flatten)]
    pub common: Common,
    #[arg(long, allow_hyphen_values = true)]
    pub text: Option<String>,
    #[arg(long, allow_hyphen_values = true)]
    pub cursor: Option<String>,
    #[arg(long, value_parser = parse_kind, action = ArgAction::Append)]
    pub kind: Vec<NodeType>,
    #[arg(long)]
    pub limit: Option<usize>,
}

#[derive(Parser)]
#[command(name = "provenance", about = "Search records in one scope")]
pub(super) struct SearchCommand {
    #[arg(value_parser = ["search"])]
    pub command: String,
    #[command(flatten)]
    pub args: SearchArgs,
}

#[derive(Args)]
pub struct DiscussionsArgs {
    pub discussion_id: Option<String>,
    #[arg(value_parser = ["get"])]
    pub action: Option<String>,
    #[command(flatten)]
    pub common: Common,
}

#[derive(Args)]
pub struct ApiArgs {
    #[command(flatten)]
    pub common: Common,
    pub path: Option<String>,
    #[arg(long, value_parser = parse_api_method, default_value = "get")]
    pub method: provenance_porcelain::api::ApiMethod,
    #[arg(long = "input", allow_hyphen_values = true)]
    pub input: Option<String>,
    #[arg(long = "header")]
    pub headers: Vec<String>,
    #[arg(long = "query")]
    pub queries: Vec<String>,
}

fn parse_api_method(value: &str) -> Result<provenance_porcelain::api::ApiMethod, String> {
    provenance_porcelain::api::ApiMethod::parse(value)
        .ok_or_else(|| "unsupported method".to_owned())
}

#[derive(Parser)]
#[command(
    name = "provenance",
    about = "Call one public API path or list the catalog routes"
)]
pub(super) struct ApiCommand {
    #[arg(value_parser = ["api"])]
    pub command: String,
    #[command(flatten)]
    pub args: ApiArgs,
}

#[derive(Parser)]
#[command(name = "provenance", about = "List or read addressed Discussions")]
pub(super) struct DiscussionsCommand {
    #[arg(value_parser = ["discussions"])]
    pub command: String,
    #[command(flatten)]
    pub args: DiscussionsArgs,
}

#[derive(Parser)]
#[command(name = "provenance", about = "Work with records in one scope")]
pub struct CatalogArgs {
    #[command(flatten)]
    pub common: Common,
    pub collection: String,
    #[arg(num_args = 0..)]
    pub address: Vec<String>,
    #[arg(long)]
    pub stdin: bool,
}

impl CatalogArgs {
    pub fn from_matches(matches: &ArgMatches) -> Self {
        Self::from_arg_matches(matches).unwrap_or_else(|error| error.exit())
    }
}

#[derive(Parser)]
#[command(name = "provenance", about = "Read or change one record")]
pub struct TargetArgs {
    #[command(flatten)]
    pub common: Common,
    pub target: String,
    #[arg(value_parser = target_actions())]
    pub action: Option<TargetVerb>,
    #[arg(long = "type", value_parser = parse_kind)]
    pub record_type: Option<NodeType>,
    #[arg(long, value_parser = get_views())]
    pub view: Option<String>,
    #[arg(long)]
    pub depth: Option<usize>,
    #[arg(long, value_parser = parse_kind, action = ArgAction::Append)]
    pub kind: Vec<NodeType>,
    #[arg(long)]
    pub limit: Option<usize>,
    #[arg(long)]
    pub stdin: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetVerb {
    Get,
    Action(Action),
}

impl TargetVerb {
    pub fn parse(word: &str) -> Option<Self> {
        if word == "get" {
            Some(Self::Get)
        } else {
            Action::parse(word).map(Self::Action)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DiscussionsRoute {
    Addressed,
    LegacyRecord,
}

pub(super) fn discussions_route(arguments: &[String], command: &Command) -> DiscussionsRoute {
    let legacy = match positional_word(arguments, command, 2).and_then(TargetVerb::parse) {
        Some(TargetVerb::Get) => true,
        Some(TargetVerb::Action(action)) => Action::RECORD.contains(&action),
        None => false,
    };
    if legacy {
        DiscussionsRoute::LegacyRecord
    } else {
        DiscussionsRoute::Addressed
    }
}

fn target_actions() -> impl TypedValueParser<Value = TargetVerb> {
    PossibleValuesParser::new(std::iter::once("get").chain(Action::ALL.map(Action::as_str)))
        .map(|word| TargetVerb::parse(&word).expect("declared target verb"))
}

fn get_views() -> PossibleValuesParser {
    PossibleValuesParser::new(View::ALL.map(View::as_str))
}

impl TargetArgs {
    pub fn from_matches(matches: &ArgMatches) -> Self {
        Self::from_arg_matches(matches).unwrap_or_else(|error| error.exit())
    }
}

fn parse_kind(value: &str) -> Result<NodeType, String> {
    NodeType::parse(value).map_err(|_| "unsupported record type".to_owned())
}

/// Find the first command word using only the declared shared option arity.
pub(super) fn command_word(arguments: &[String]) -> Option<&str> {
    let common = Common::augment_args(Command::new("provenance"));
    positional_word(arguments, &common, 1)
}

/// Read a positional word using the selected command's declared option arity.
pub(super) fn positional_word<'a>(
    arguments: &'a [String],
    command: &Command,
    position: usize,
) -> Option<&'a str> {
    let mut index = 1;
    let mut seen = 0;
    while let Some(word) = arguments.get(index) {
        if word == "--" {
            return arguments.get(index + position - seen).map(String::as_str);
        }
        if let Some(option) = word.strip_prefix("--") {
            let (name, assigned) = option
                .split_once('=')
                .map_or((option, false), |(name, _)| (name, true));
            let declaration = command
                .get_arguments()
                .find(|arg| arg.get_long() == Some(name))?;
            let takes_value = !matches!(declaration.get_action(), &ArgAction::SetTrue);
            index += 1 + usize::from(takes_value && !assigned);
            continue;
        }
        if word.starts_with('-') {
            return None;
        }
        seen += 1;
        if seen == position {
            return Some(word);
        }
        index += 1;
    }
    None
}

pub fn target_command() -> Command {
    TargetArgs::command()
}

pub fn catalog_command() -> Command {
    CatalogArgs::command()
}

pub fn discussions_command() -> anyhow::Result<Command> {
    crate::catalog_cli::fields::augment_schemas(
        DiscussionsCommand::command(),
        [
            provenance_porcelain::discussion::input_schema(Action::Discussions),
            provenance_porcelain::discussion::input_schema(Action::Discussion),
        ],
        &["parent", "discussion_id"],
    )
}
