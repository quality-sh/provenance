//! One declaration for each porcelain command.
//!
//! A declaration names a command, describes it, and declares its one input: the fields,
//! the fields that name its target, and the relations between them. It does no work:
//! the handlers stay behind the existing ports. Each consumer keeps its own list of the
//! commands that it shows, and shows each one from this declaration by one rule.

mod dogfood;
mod fields;
mod graph;
mod porcelain;
mod porcelain_fields;
mod repository;
mod workspace;

pub use crate::cargo_init::CargoInitInput;
pub use crate::dictionary::{DictionaryImportInput, DictionaryStatusInput};
pub use crate::export::ExportInput;
pub use crate::graph_reference::{ReferenceInput, ReferenceIssueInput};
pub use crate::import::ImportInput;
pub use crate::init::InitInput;
pub use crate::materialize::MaterializeInput;
pub use crate::report::{ReportBuildInput, ReportRenderInput};
pub use crate::schema::SchemaShowInput;
pub use crate::swarm_backtrace::SwarmBacktraceLandInput;
pub use crate::validate::ValidateInput;
pub use dogfood::{
    DogfoodCommand, DogfoodListInput, DogfoodNoteInput, DogfoodReportInput, DogfoodSimulateInput,
    TriageHandleInput, TriageListInput, TriageReopenInput,
};
pub use fields::{json_format, Field, FieldValue, Fields, Relation, REPO, SCOPE};
pub use graph::GraphCommand;
pub use porcelain::{PrimeFormat, PrimeInput};
pub use repository::{MergeJsonlInput, RepositoryCommand};
pub use workspace::{SkillsInstallInput, SkillsListInput, SkillsShowInput, WorkspaceCommand};

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
    /// A typed input, declared as its fields.
    Fields(Fields),
    /// The fields of the catalog operations that the action selects, after these fields
    /// of the action.
    RecordFields {
        action: Action,
        fields: &'static [Field],
        relations: &'static [Relation],
    },
    /// The typed input of one Discussion action.
    Discussion(Action),
    None,
}

impl Input {
    /// The declared fields, in order.
    pub const fn fields(&self) -> &'static [Field] {
        match self {
            Self::Fields(fields) => fields.fields,
            Self::RecordFields { fields, .. } => fields,
            Self::Discussion(_) | Self::None => &[],
        }
    }

    /// The relations between the declared fields.
    pub const fn relations(&self) -> &'static [Relation] {
        match self {
            Self::Fields(fields) => fields.relations,
            Self::RecordFields { relations, .. } => relations,
            Self::Discussion(_) | Self::None => &[],
        }
    }

    /// The fields that name what the command acts on, in order.
    pub const fn target(&self) -> &'static [&'static str] {
        match self {
            Self::Fields(fields) => fields.target,
            Self::RecordFields { .. } | Self::Discussion(_) | Self::None => &[],
        }
    }
}

/// What one command is. It holds no handler and names no consumer.
#[rule("rule_porcelain_names_no_consumer")]
#[derive(Clone, Copy, Debug)]
pub struct Declaration {
    /// The name. Its words select the command when its grammar takes a name.
    pub name: &'static str,
    /// The description. An empty description shows no about line.
    pub description: &'static str,
    pub grammar: Grammar,
    pub input: Input,
    /// Programs run this command, so a consumer does not document it for people.
    pub internal: bool,
    /// What the command runs on that the consumer binds, so its fields do not name it.
    pub bound: Binding,
}

/// What of its place a command takes from the consumer, not from its fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Binding {
    /// The fields give all that the command reads.
    None,
    /// The command reads the files of the bound repository and no scope.
    Repository,
    /// The command works on the bound repository and scope.
    Scope,
}

impl Declaration {
    /// A command selected by its name, with a typed input.
    pub const fn named(name: &'static str, description: &'static str, fields: Fields) -> Self {
        Self {
            name,
            description,
            grammar: Grammar::Name,
            input: Input::Fields(fields),
            internal: false,
            bound: Binding::None,
        }
    }

    /// This command, on the repository and scope that the consumer binds.
    #[must_use]
    pub const fn bound(mut self) -> Self {
        self.bound = Binding::Scope;
        self
    }

    /// This command, on the files of the repository that the consumer binds.
    #[must_use]
    pub const fn bound_repository(mut self) -> Self {
        self.bound = Binding::Repository;
        self
    }

    /// This command, run by programs.
    #[must_use]
    pub const fn internal(mut self) -> Self {
        self.internal = true;
        self
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
    /// The command that runs one record action, selected by its action argument.
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

    /// Implementation aid: each typed input has one schema property for each field.
    #[test]
    fn each_typed_input_has_a_property_for_each_field() {
        for command in Command::ALL {
            let Input::Fields(fields) = command.declaration().input else {
                continue;
            };
            let Some(schema) = fields.schema else {
                continue;
            };
            let target = command
                .declaration()
                .grammar
                .takes_target()
                .then_some("target");
            let mut names = fields
                .fields
                .iter()
                .map(|field| field.name)
                .chain(target)
                .collect::<Vec<_>>();
            names.sort_unstable();
            let schema = schema();
            let mut properties = schema["properties"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>();
            properties.sort_unstable();
            assert_eq!(names, properties, "{command:?}");
        }
    }

    /// Implementation aid: each relation and each target names a field of its command.
    #[test]
    fn relations_and_targets_name_declared_fields() {
        for command in Command::ALL {
            let input = command.declaration().input;
            let declared = |name: &str| input.fields().iter().any(|field| field.name == name);
            for relation in input.relations() {
                let (Relation::Requires(first, second) | Relation::Conflicts(first, second)) =
                    *relation;
                assert!(
                    declared(first) && declared(second),
                    "{command:?}: {relation:?}"
                );
            }
            for target in input.target() {
                assert!(declared(target), "{command:?}: {target}");
            }
        }
    }

    /// Implementation aid: each command has its own declaration name.
    #[test]
    fn each_declaration_name_selects_its_command() {
        for command in Command::ALL {
            assert_eq!(Command::named(command.declaration().name), Some(*command));
        }
    }
}
