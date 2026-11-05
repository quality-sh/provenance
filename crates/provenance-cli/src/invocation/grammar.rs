use super::GlobalContext;
use crate::output::JsonFormat;
use clap::{
    builder::{PossibleValuesParser, TypedValueParser},
    Arg, ArgAction, ArgMatches, Command,
};
use provenance_cli::porcelain;
use provenance_cli::repo_context::RepoContext;
use provenance_core::NodeType;
use provenance_porcelain::action::Action;
use provenance_porcelain::registry::{self, CliForm};

/// The shared options of the porcelain and catalog grammars, read from the registry's
/// global flags.
pub struct Common {
    pub repo: String,
    pub scope: String,
    pub format: Option<JsonFormat>,
    pub quiet: bool,
}

impl Common {
    pub fn from_matches(matches: &ArgMatches) -> Self {
        let text = |name: &str| matches.get_one::<String>(name).cloned().unwrap_or_default();
        Self {
            repo: text("repo"),
            scope: text("scope"),
            format: matches
                .get_one::<String>("format")
                .and_then(|format| JsonFormat::from_flag(format)),
            quiet: matches.get_flag("quiet"),
        }
    }

    pub fn context(&self) -> GlobalContext {
        GlobalContext {
            repo: RepoContext::new(&self.repo, &self.scope),
            quiet: self.quiet,
        }
    }

    pub fn format(&self) -> Option<porcelain::OutputFormat> {
        self.format.map(|_| porcelain::OutputFormat::Json)
    }
}

/// The search flags, read from the registry-rendered search parser.
pub struct SearchArgs {
    pub common: Common,
    pub text: Option<String>,
    pub cursor: Option<String>,
    pub kind: Vec<NodeType>,
    pub limit: Option<usize>,
    pub include_terminal: bool,
    pub file: Option<String>,
    pub symbol: Option<String>,
    pub line: Option<usize>,
}

impl SearchArgs {
    pub fn from_matches(matches: &ArgMatches) -> Self {
        Self {
            common: Common::from_matches(matches),
            text: matches.get_one::<String>("text").cloned(),
            cursor: matches.get_one::<String>("cursor").cloned(),
            kind: kinds(matches),
            limit: matches.get_one::<usize>("limit").copied(),
            include_terminal: matches.get_flag("include_terminal"),
            file: matches.get_one::<String>("file").cloned(),
            symbol: matches.get_one::<String>("symbol").cloned(),
            line: matches.get_one::<usize>("line").copied(),
        }
    }
}

fn kinds(matches: &ArgMatches) -> Vec<NodeType> {
    matches
        .get_many::<NodeType>("kind")
        .into_iter()
        .flatten()
        .copied()
        .collect()
}

/// The `provenance discussions` flags, read from the discussions parser.
pub struct DiscussionsArgs {
    pub discussion_id: Option<String>,
    pub action: Option<String>,
    pub common: Common,
}

impl DiscussionsArgs {
    pub fn from_matches(matches: &ArgMatches) -> Self {
        Self {
            discussion_id: matches.get_one::<String>("discussion_id").cloned(),
            action: matches.get_one::<String>("action").cloned(),
            common: Common::from_matches(matches),
        }
    }
}

/// The api flags, read from the registry-rendered api parser.
pub struct ApiArgs {
    pub common: Common,
    pub path: Option<String>,
    pub method: Option<provenance_porcelain::api::ApiMethod>,
    /// The JSON body source: a file, or standard input with "-".
    pub input: Option<String>,
    pub headers: Vec<String>,
    pub queries: Vec<String>,
    pub filter: Option<String>,
    pub limit: Option<usize>,
}

impl ApiArgs {
    pub fn from_matches(matches: &ArgMatches) -> Self {
        let many = |name: &str| {
            matches
                .get_many::<String>(name)
                .into_iter()
                .flatten()
                .cloned()
                .collect()
        };
        Self {
            common: Common::from_matches(matches),
            path: matches.get_one::<String>("path").cloned(),
            method: matches
                .get_one::<provenance_porcelain::api::ApiMethod>("method")
                .copied(),
            input: matches.get_one::<String>("input").cloned(),
            headers: many("header"),
            queries: many("query"),
            filter: matches.get_one::<String>("filter").cloned(),
            limit: matches.get_one::<usize>("limit").copied(),
        }
    }
}

/// The target, its action, and the get flags, read from the target parser.
pub struct TargetArgs {
    pub common: Common,
    pub target: String,
    pub action: Option<TargetVerb>,
    pub record_type: Option<NodeType>,
    pub view: Option<String>,
    pub depth: Option<usize>,
    pub kind: Vec<NodeType>,
    pub limit: Option<usize>,
    pub delivery: Delivery,
}

/// Selects the target read's delivery, while keeping CLI flag names unchanged.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Delivery {
    #[default]
    Read,
    Follow,
    ReviewLink {
        open: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetVerb {
    Get,
    /// The get trace view as an action.
    Trace,
    Action(Action),
}

impl TargetVerb {
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "get" => Some(Self::Get),
            "trace" => Some(Self::Trace),
            _ => Action::parse(word).map(Self::Action),
        }
    }

    /// The CLI word of this verb.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Trace => "trace",
            Self::Action(action) => action.as_str(),
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
        Some(TargetVerb::Trace) | None => false,
    };
    if legacy {
        DiscussionsRoute::LegacyRecord
    } else {
        DiscussionsRoute::Addressed
    }
}

fn target_actions() -> impl TypedValueParser<Value = TargetVerb> {
    PossibleValuesParser::new(target_words())
        .map(|word| TargetVerb::parse(&word).expect("declared target verb"))
}

/// The registry commands that take a target, in registry order.
fn target_words() -> Vec<&'static str> {
    target_declarations()
        .map(|declaration| declaration.name)
        .collect()
}

fn target_declarations() -> impl Iterator<Item = registry::Declaration> {
    registry::Command::ALL
        .iter()
        .map(|command| command.declaration())
        .filter(|declaration| {
            matches!(
                declaration.cli_form,
                CliForm::Target | CliForm::WordsAndTarget(_)
            )
        })
}

impl TargetArgs {
    pub fn from_matches(matches: &ArgMatches) -> Self {
        let mut args = Self {
            common: Common::from_matches(matches),
            target: matches
                .get_one::<String>("target")
                .cloned()
                .unwrap_or_default(),
            action: matches.get_one::<TargetVerb>("action").copied(),
            record_type: matches.get_one::<NodeType>("record_type").copied(),
            view: matches.get_one::<String>("view").cloned(),
            depth: matches.get_one::<usize>("depth").copied(),
            kind: kinds(matches),
            limit: matches.get_one::<usize>("limit").copied(),
            delivery: Delivery::Read,
        };
        args.delivery = if matches.get_flag("follow") {
            Delivery::Follow
        } else if matches.get_flag("review_link") {
            Delivery::ReviewLink {
                open: !matches.get_flag("no_open"),
            }
        } else {
            Delivery::Read
        };
        args
    }
}

/// Find the first command word using only the declared shared option arity.
pub(super) fn command_word(arguments: &[String]) -> Option<&str> {
    let common = crate::registry_cli::with_global_flags(Command::new("provenance"));
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

/// The target parser: the target, its action, and the flags of every target command.
pub fn target_command() -> Command {
    let mut command = crate::registry_cli::with_global_flags(Command::new("provenance"))
        .about("Read or change one record")
        .arg(Arg::new("target").required(true).value_name("TARGET"))
        .arg(
            Arg::new("action")
                .value_parser(target_actions())
                .value_name("ACTION"),
        );
    let mut declared = std::collections::BTreeSet::new();
    for declaration in target_declarations() {
        for (flag, arg) in declaration
            .flags
            .iter()
            .zip(crate::registry_cli::args(&declaration))
        {
            if declared.insert(flag.field_id()) {
                command = command.arg(arg);
            }
        }
    }
    command
}

pub fn discussions_command() -> anyhow::Result<Command> {
    let command = Command::new("provenance")
        .about(
            registry::Command::Action(Action::Discussions)
                .declaration()
                .description,
        )
        .arg(
            Arg::new("command")
                .required(true)
                .value_name("COMMAND")
                .value_parser(["discussions"]),
        )
        .arg(Arg::new("discussion_id").value_name("DISCUSSION_ID"))
        .arg(
            Arg::new("action")
                .value_name("ACTION")
                .value_parser(["get"]),
        );
    crate::registry_cli::augment_schemas(
        crate::registry_cli::with_global_flags(command),
        [
            provenance_porcelain::discussion::input_schema(Action::Discussions),
            provenance_porcelain::discussion::input_schema(Action::Discussion),
        ],
        &["parent", "discussion_id"],
    )
}
