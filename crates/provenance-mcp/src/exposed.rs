//! The porcelain commands that MCP shows, each with the place that shows it.

use provenance_porcelain::action::Action;
use provenance_porcelain::registry::{Command, GraphCommand, RepositoryCommand};
use std::sync::LazyLock;

/// Where MCP shows one command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Place {
    /// The command is its own tool, with the command name.
    Tool,
    /// The command is one value of the action argument of this tool.
    Action { tool: &'static str },
    /// The command is one value of the categories argument of this tool.
    Category { tool: &'static str },
}

/// Each porcelain command that MCP shows. The tools are in tools/list order.
pub const EXPOSED: &[(Command, Place)] = &[
    (Command::Get, Place::Tool),
    (Command::Search, Place::Tool),
    (Command::Api, Place::Tool),
    (Command::Check, Place::Tool),
    (Command::Trace, Place::Tool),
    (Command::Change, Place::Tool),
    (Command::Action(Action::Discussions), Place::Tool),
    (Command::Action(Action::Create), CHANGE),
    (Command::Action(Action::Update), CHANGE),
    (Command::Action(Action::Answer), CHANGE),
    (Command::Action(Action::Claim), CHANGE),
    (Command::Action(Action::Release), CHANGE),
    (Command::Action(Action::Submit), CHANGE),
    (Command::Action(Action::Discuss), CHANGE),
    (Command::Action(Action::Reply), CHANGE),
    (Command::Repository(RepositoryCommand::Materialize), CHANGE),
    (Command::Repository(RepositoryCommand::Import), CHANGE),
    (
        Command::Repository(RepositoryCommand::DictionaryImport),
        CHANGE,
    ),
    (Command::Graph(GraphCommand::SwarmBacktraceLand), CHANGE),
    (
        Command::Action(Action::Discussion),
        Place::Action {
            tool: "discussions",
        },
    ),
];

const CHANGE: Place = Place::Action { tool: "change" };

/// The commands that MCP shows as their own tool, in tools/list order.
pub fn tools() -> impl Iterator<Item = Command> {
    EXPOSED
        .iter()
        .filter(|(_, place)| *place == Place::Tool)
        .map(|(command, _)| *command)
}

/// The MCP spelling of each shown command name.
static NAMES: LazyLock<Vec<(Command, String)>> = LazyLock::new(|| {
    EXPOSED
        .iter()
        .map(|(command, _)| (*command, spelling(command.declaration().name)))
        .collect()
});

/// The MCP name of one shown command: the words of the command name in `snake_case`,
/// so `dictionary import` is `dictionary_import`. The words stay the same.
pub fn name(command: Command) -> &'static str {
    NAMES
        .iter()
        .find(|(shown, _)| *shown == command)
        .map_or_else(
            || panic!("{command:?} is not shown on MCP"),
            |(_, name)| name.as_str(),
        )
}

/// The `snake_case` spelling of a command name.
pub fn spelling(name: &str) -> String {
    name.replace([' ', '-'], "_")
}
