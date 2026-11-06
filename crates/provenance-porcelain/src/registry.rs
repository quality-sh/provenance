//! One declaration for each porcelain command.
//!
//! A declaration names a command, describes it, and declares its input. It does no
//! work: the handlers stay behind the existing ports. Each consumer keeps its own list
//! of the commands that it shows, and its own grammar for them.

mod dogfood;
mod graph;
mod porcelain;
mod repository;
mod workspace;

pub use dogfood::{
    DogfoodCommand, DogfoodListInput, DogfoodNoteInput, DogfoodReportInput, DogfoodSimulateInput,
    TriageHandleInput, TriageListInput, TriageReopenInput,
};
pub use graph::{GraphCommand, ReferenceInput, ReferenceIssueInput, SwarmBacktraceLandInput};
pub use porcelain::{PrimeFormat, PrimeInput};
pub use repository::{
    CargoInitInput, DictionaryImportInput, DictionaryStatusInput, ExportInput, ImportInput,
    InitInput, MaterializeInput, MergeJsonlInput, RepositoryCommand, SchemaShowInput,
    ValidateInput,
};
pub use workspace::{
    DocsCheckInput, ReportBuildInput, ReportRenderInput, SkillsInstallInput, SkillsListInput,
    SkillsShowInput, WorkspaceCommand,
};

use crate::action::Action;
use provenance_macros::rule;

/// Whether a command takes a target record before its name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Grammar {
    /// The command is selected by the words of its name.
    Name,
    /// The command acts on one target record.
    Target,
    /// The command is selected by its name, or acts on one target record.
    NameAndTarget,
}

impl Grammar {
    /// Whether the command can act on a target record.
    pub const fn takes_target(self) -> bool {
        matches!(self, Self::Target | Self::NameAndTarget)
    }

    /// Whether the words of the name select the command.
    pub const fn takes_name(self) -> bool {
        matches!(self, Self::Name | Self::NameAndTarget)
    }
}

/// The input of a command.
#[derive(Clone, Copy, Debug)]
pub enum Input {
    /// A typed input, given as its JSON Schema.
    Typed(fn() -> serde_json::Value),
    /// The fields of the catalog operations that the action selects.
    RecordFields(Action),
    /// The typed input of one Discussion action.
    Discussion(Action),
    None,
}

/// What one command is. It holds no handler and names no consumer: each consumer
/// keeps its own grammar for the command.
#[rule("rule_porcelain_names_no_consumer")]
#[derive(Clone, Copy, Debug)]
pub struct Declaration {
    /// The name. Its words select the command when its grammar takes a name.
    pub name: &'static str,
    /// The description. An empty description shows no about line.
    pub description: &'static str,
    pub grammar: Grammar,
    pub input: Input,
}

impl Declaration {
    /// A command selected by its name, with a typed input.
    pub(crate) const fn named(
        name: &'static str,
        description: &'static str,
        schema: fn() -> serde_json::Value,
    ) -> Self {
        Self {
            name,
            description,
            grammar: Grammar::Name,
            input: Input::Typed(schema),
        }
    }

    /// The words of the name.
    pub fn words(&self) -> impl Iterator<Item = &'static str> {
        self.name.split(' ')
    }
}

/// Every porcelain command.
#[rule("rule_porcelain_registry_defines_commands")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    Get,
    Search,
    Api,
    Check,
    /// The records above one record, up to its Sources: the get trace view as an action.
    Trace,
    Prime,
    /// The MCP tool that runs one record action, selected by its action argument.
    Change,
    Action(Action),
    Repository(RepositoryCommand),
    Graph(GraphCommand),
    Workspace(WorkspaceCommand),
    /// Dev-build-only commands.
    Dogfood(DogfoodCommand),
}

impl Command {
    /// Every command.
    pub const ALL: &'static [Self] = &[
        Self::Get,
        Self::Search,
        Self::Api,
        Self::Check,
        Self::Trace,
        Self::Prime,
        Self::Change,
        Self::Action(Action::Create),
        Self::Action(Action::Update),
        Self::Action(Action::Answer),
        Self::Action(Action::Claim),
        Self::Action(Action::Release),
        Self::Action(Action::Submit),
        Self::Action(Action::Discussions),
        Self::Action(Action::Discussion),
        Self::Action(Action::Discuss),
        Self::Action(Action::Reply),
        Self::Repository(RepositoryCommand::Init),
        Self::Repository(RepositoryCommand::CargoInit),
        Self::Repository(RepositoryCommand::Materialize),
        Self::Repository(RepositoryCommand::Export),
        Self::Repository(RepositoryCommand::Import),
        Self::Repository(RepositoryCommand::MergeJsonl),
        Self::Repository(RepositoryCommand::Validate),
        Self::Repository(RepositoryCommand::SchemaShow),
        Self::Repository(RepositoryCommand::DictionaryImport),
        Self::Repository(RepositoryCommand::DictionaryStatus),
        Self::Graph(GraphCommand::ReferenceIssue),
        Self::Graph(GraphCommand::ReferenceShow),
        Self::Graph(GraphCommand::ReferenceVerify),
        Self::Graph(GraphCommand::ReferenceExactExport),
        Self::Graph(GraphCommand::SwarmBacktraceLand),
        Self::Workspace(WorkspaceCommand::DocsCheck),
        Self::Workspace(WorkspaceCommand::SkillsList),
        Self::Workspace(WorkspaceCommand::SkillsShow),
        Self::Workspace(WorkspaceCommand::SkillsInstall),
        Self::Workspace(WorkspaceCommand::ReportRender),
        Self::Workspace(WorkspaceCommand::ReportBuild),
        Self::Dogfood(DogfoodCommand::Note),
        Self::Dogfood(DogfoodCommand::List),
        Self::Dogfood(DogfoodCommand::Report),
        Self::Dogfood(DogfoodCommand::TriageHandle),
        Self::Dogfood(DogfoodCommand::TriageReopen),
        Self::Dogfood(DogfoodCommand::TriageList),
        Self::Dogfood(DogfoodCommand::Simulate),
    ];

    /// The declaration of this command.
    pub fn declaration(self) -> Declaration {
        porcelain::declaration(self)
    }

    /// The command with this declaration name.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|command| command.declaration().name == name)
    }

    /// The command with this name that acts on a target record.
    pub fn find_target_action(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|command| {
            let declaration = command.declaration();
            declaration.grammar.takes_target() && declaration.name == name
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Implementation aid: each command has its own declaration name.
    #[test]
    fn each_declaration_name_selects_its_command() {
        for command in Command::ALL {
            assert_eq!(Command::named(command.declaration().name), Some(*command));
        }
    }
}
