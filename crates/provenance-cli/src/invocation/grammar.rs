use super::GlobalContext;
use crate::{output::JsonFormat, porcelain};
use clap::{Args, ArgAction, ArgMatches, Command, CommandFactory, FromArgMatches, Parser, ValueEnum};
use provenance_core::NodeType;
use provenance_porcelain::get::View;

/// Shared options for the Porcelain and catalog grammars.
#[derive(Args)]
pub(crate) struct Common {
    #[arg(long, global = true, default_value = ".", allow_hyphen_values = true)]
    pub repo: String,
    #[arg(long, global = true, default_value = "default", allow_hyphen_values = true)]
    pub scope: String,
    #[arg(long, global = true, value_enum)]
    pub format: Option<JsonFormat>,
    #[arg(long, global = true)]
    pub quiet: bool,
}

impl Common {
    pub(crate) fn context(&self) -> GlobalContext {
        GlobalContext {
            repo: self.repo.clone(),
            scope: self.scope.clone(),
            quiet: self.quiet,
        }
    }

    pub(crate) fn format(&self) -> Option<porcelain::OutputFormat> {
        self.format.map(|_| porcelain::OutputFormat::Json)
    }
}

#[derive(Parser)]
#[command(name = "provenance", about = "Search records in one scope")]
pub(super) struct SearchArgs {
    #[command(flatten)]
    pub common: Common,
    #[arg(value_parser = ["search"])]
    pub command: String,
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
#[command(name = "provenance")]
pub(crate) struct CatalogArgs {
    #[command(flatten)]
    pub common: Common,
    pub collection: String,
    #[arg(num_args = 0..)]
    pub address: Vec<String>,
    #[arg(long)]
    pub stdin: bool,
}

impl CatalogArgs {
    pub(crate) fn from_matches(matches: &ArgMatches) -> Self {
        Self::from_arg_matches(matches).unwrap_or_else(|error| error.exit())
    }
}

#[derive(Clone, Copy, Default, ValueEnum)]
pub(super) enum GetView {
    #[default]
    Record,
    Children,
    Grounding,
    Impact,
}

impl From<GetView> for View {
    fn from(value: GetView) -> Self {
        match value {
            GetView::Record => Self::Record,
            GetView::Children => Self::Children,
            GetView::Grounding => Self::Grounding,
            GetView::Impact => Self::Impact,
        }
    }
}

#[derive(Parser)]
#[command(name = "provenance")]
pub(super) struct TargetArgs {
    #[command(flatten)]
    pub common: Common,
    pub target: String,
    #[arg(value_parser = ["get", "create", "update", "answer", "claim", "release", "submit"])]
    pub action: Option<String>,
    #[arg(long = "type")]
    pub record_type: Option<String>,
    #[arg(long, value_enum)]
    pub view: Option<GetView>,
    #[arg(long)]
    pub depth: Option<usize>,
    #[arg(long, value_parser = parse_kind, action = ArgAction::Append)]
    pub kind: Vec<NodeType>,
    #[arg(long)]
    pub limit: Option<usize>,
    #[arg(long)]
    pub stdin: bool,
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
    let mut index = 1;
    while let Some(word) = arguments.get(index) {
        if word == "--" {
            return arguments.get(index + 1).map(String::as_str);
        }
        if let Some(option) = word.strip_prefix("--") {
            let (name, assigned) = option.split_once('=').map_or((option, false), |(name, _)| (name, true));
            let declaration = common.get_arguments().find(|arg| arg.get_long() == Some(name))?;
            let takes_value = declaration.get_action() != &ArgAction::SetTrue;
            index += 1 + usize::from(takes_value && !assigned);
            continue;
        }
        if word.starts_with('-') {
            return None;
        }
        return Some(word);
    }
    None
}

pub(crate) fn target_command() -> Command {
    TargetArgs::command()
}

pub(crate) fn catalog_command() -> Command {
    CatalogArgs::command()
}
