//! The commands that the CLI shows.

use crate::servers::SERVERS;
use provenance_macros::rule;
use provenance_porcelain::action::Action;
use provenance_porcelain::guidance::Shown;
#[cfg(feature = "dogfood")]
use provenance_porcelain::registry::DogfoodCommand;
use provenance_porcelain::registry::{
    Command, Declaration, GraphCommand, RepositoryCommand, WorkspaceCommand,
};

/// Each porcelain command that the CLI shows, in registry order. The dogfood commands
/// are only in builds with the `dogfood` feature.
#[rule("rule_porcelain_interface_records_each_command")]
pub const EXPOSED: &[Command] = &[
    Command::Get,
    Command::Search,
    Command::Api,
    Command::Check,
    Command::Trace,
    Command::Prime,
    Command::Action(Action::Create),
    Command::Action(Action::Update),
    Command::Action(Action::Answer),
    Command::Action(Action::Claim),
    Command::Action(Action::Release),
    Command::Action(Action::Submit),
    Command::Action(Action::Discussions),
    Command::Action(Action::Discussion),
    Command::Action(Action::Discuss),
    Command::Action(Action::Reply),
    Command::Repository(RepositoryCommand::Init),
    Command::Repository(RepositoryCommand::CargoInit),
    Command::Repository(RepositoryCommand::Materialize),
    Command::Repository(RepositoryCommand::Export),
    Command::Repository(RepositoryCommand::Import),
    Command::Repository(RepositoryCommand::MergeJsonl),
    Command::Repository(RepositoryCommand::Validate),
    Command::Repository(RepositoryCommand::SchemaShow),
    Command::Repository(RepositoryCommand::DictionaryImport),
    Command::Repository(RepositoryCommand::DictionaryStatus),
    Command::Graph(GraphCommand::SwarmBacktraceLand),
    Command::Workspace(WorkspaceCommand::SkillsList),
    Command::Workspace(WorkspaceCommand::SkillsShow),
    Command::Workspace(WorkspaceCommand::SkillsInstall),
    Command::Workspace(WorkspaceCommand::ReportRender),
    Command::Workspace(WorkspaceCommand::ReportBuild),
    #[cfg(feature = "dogfood")]
    Command::Dogfood(DogfoodCommand::Note),
    #[cfg(feature = "dogfood")]
    Command::Dogfood(DogfoodCommand::List),
    #[cfg(feature = "dogfood")]
    Command::Dogfood(DogfoodCommand::Report),
    #[cfg(feature = "dogfood")]
    Command::Dogfood(DogfoodCommand::TriageHandle),
    #[cfg(feature = "dogfood")]
    Command::Dogfood(DogfoodCommand::TriageReopen),
    #[cfg(feature = "dogfood")]
    Command::Dogfood(DogfoodCommand::TriageList),
    #[cfg(feature = "dogfood")]
    Command::Dogfood(DogfoodCommand::Simulate),
];

/// Whether the CLI shows `command`.
pub fn shows(command: Command) -> bool {
    EXPOSED.contains(&command)
}

/// Each form that selects the command of `declaration` on the CLI, joined by ` or `.
///
/// The forms are the words of its name, when its name selects it, and `<id>` and its name,
/// when it acts on a record, as in `<id> update`.
pub fn form(declaration: &Declaration) -> String {
    let named = crate::cli_options::words(declaration).map(|words| words.join(" "));
    let on_record = declaration
        .grammar
        .takes_target()
        .then(|| format!("<id> {}", declaration.name));
    named
        .into_iter()
        .chain(on_record)
        .collect::<Vec<_>>()
        .join(" or ")
}

/// Each command that the CLI shows, with its CLI form.
///
/// The order is the order of [`EXPOSED`], with the server commands after the workspace
/// commands and before the dogfood commands. The root help gives the commands that a name
/// selects in this order.
pub fn shown() -> Vec<(Shown, String)> {
    let (dogfood, porcelain): (Vec<Command>, Vec<Command>) = EXPOSED
        .iter()
        .copied()
        .partition(|command| matches!(command, Command::Dogfood(_)));
    let servers = SERVERS
        .iter()
        .map(|server| Shown::Declared(server.declaration()));
    porcelain
        .into_iter()
        .map(Shown::Registry)
        .chain(servers)
        .chain(dogfood.into_iter().map(Shown::Registry))
        .map(|shown| (shown, form(&shown.declaration())))
        .collect()
}

/// The porcelain command that the CLI root words select. A target action is not found
/// by its words.
pub fn find(words: &[&str]) -> Option<Command> {
    EXPOSED.iter().copied().find(|command| {
        crate::cli_options::words(&command.declaration()).is_some_and(|selected| selected == words)
    })
}
