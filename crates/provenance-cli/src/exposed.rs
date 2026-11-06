//! The porcelain commands that the CLI shows.

use provenance_macros::rule;
use provenance_porcelain::action::Action;
#[cfg(feature = "dogfood")]
use provenance_porcelain::registry::DogfoodCommand;
use provenance_porcelain::registry::{Command, GraphCommand, RepositoryCommand, WorkspaceCommand};

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
    Command::Graph(GraphCommand::ReferenceIssue),
    Command::Graph(GraphCommand::ReferenceShow),
    Command::Graph(GraphCommand::ReferenceVerify),
    Command::Graph(GraphCommand::ReferenceExactExport),
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

/// The porcelain command that the CLI root words select. A target action is not found
/// by its words.
pub fn find(words: &[&str]) -> Option<Command> {
    EXPOSED.iter().copied().find(|command| {
        crate::cli_form::declaration(*command)
            .words()
            .is_some_and(|selected| selected == words)
    })
}
