//! One declaration for each command on the CLI and each MCP tool.
//!
//! A declaration names a command, describes it, and declares its inputs for each
//! interface. It does no work: the handlers stay behind the existing ports. The CLI
//! renders its parser from these declarations and the MCP server lists its tools
//! from them.

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
pub use porcelain::{
    CheckCliInput, CheckFormat, GetCliInput, PrimeFormat, PrimeInput, ReportFileFormat,
};
pub use repository::{
    CargoInitInput, DictionaryImportInput, DictionaryStatusInput, ExportInput, ImportInput,
    InitInput, MaterializeInput, MergeJsonlInput, RepositoryCommand, SchemaShowInput,
    ValidateInput,
};
pub use workspace::{
    DocsCheckInput, DocsServeInput, McpInput, ReportBuildInput, ReportRenderInput, ReviewInput,
    SkillsInstallInput, SkillsListInput, SkillsShowInput, WorkspaceCommand,
};

use crate::action::Action;
use provenance_macros::rule;

/// One interface that shows commands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Interface {
    Cli,
    Mcp,
}

/// Whether an interface shows a command, with the reason when it does not.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Presence {
    Present,
    Absent { reason: &'static str },
}

/// The CLI words that select a command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CliForm {
    /// `provenance <words...>`.
    Words(&'static [&'static str]),
    /// `provenance <target> <name>`.
    Target,
    /// Both the words and the target form.
    WordsAndTarget(&'static [&'static str]),
}

/// The input of a command on one interface.
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

/// A relation between two CLI flags of one command, by input field name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Relation {
    /// The first flag needs the second flag.
    Requires(&'static str, &'static str),
    /// The two flags cannot be given together.
    Conflicts(&'static str, &'static str),
}

/// How a CLI flag takes its value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlagKind {
    /// `--name` without a value.
    Switch,
    /// `--name <value>`, at most once.
    Value,
    /// `--name <value>`, zero or more times.
    Repeated,
    /// A value by position.
    Positional,
}

/// The value type of a CLI flag, so the CLI refuses a bad value before the command runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlagValue {
    Text,
    /// A whole number from zero.
    Count,
    /// One record kind.
    RecordKind,
    /// One api method.
    ApiMethod,
    /// An api catalog page size from 1 through 200.
    CatalogLimit,
    /// A TCP port number.
    Port,
}

/// One CLI flag of a command, in the order that help shows it.
///
/// The JSON Schema of a typed input does not keep field order or the CLI-only facts
/// below, so each command declares its flags here.
// Each bool is one independent CLI fact of the flag.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Flag {
    /// The CLI flag name, without `--`.
    pub name: &'static str,
    /// The input field, when it is not the name with `_` for `-`.
    pub field_name: Option<&'static str>,
    pub kind: FlagKind,
    pub value: FlagValue,
    pub help: Option<&'static str>,
    pub default: Option<&'static str>,
    /// The accepted values, when the flag takes one of a fixed set.
    pub values: &'static [&'static str],
    /// The value can start with `-`.
    pub hyphen_values: bool,
    pub hidden: bool,
    /// The command refuses to run without this flag.
    pub required: bool,
    /// The subcommands after this flag also take it.
    pub global: bool,
}

impl Flag {
    /// A flag with no help, default, or value set.
    pub const fn new(name: &'static str, kind: FlagKind) -> Self {
        Self {
            name,
            field_name: None,
            kind,
            value: FlagValue::Text,
            help: None,
            default: None,
            values: &[],
            hyphen_values: false,
            hidden: false,
            required: false,
            global: false,
        }
    }

    #[must_use]
    pub const fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    #[must_use]
    pub const fn required(mut self) -> Self {
        self.required = true;
        self
    }

    #[must_use]
    pub const fn global(mut self) -> Self {
        self.global = true;
        self
    }

    #[must_use]
    pub const fn value(mut self, value: FlagValue) -> Self {
        self.value = value;
        self
    }

    #[must_use]
    pub const fn help(mut self, help: &'static str) -> Self {
        self.help = Some(help);
        self
    }

    #[must_use]
    pub const fn default(mut self, default: &'static str) -> Self {
        self.default = Some(default);
        self
    }

    #[must_use]
    pub const fn values(mut self, values: &'static [&'static str]) -> Self {
        self.values = values;
        self
    }

    #[must_use]
    pub const fn hyphen_values(mut self) -> Self {
        self.hyphen_values = true;
        self
    }

    #[must_use]
    pub const fn field(mut self, field: &'static str) -> Self {
        self.field_name = Some(field);
        self
    }

    /// The input field name. It is also the CLI argument ID.
    pub fn field_id(&self) -> String {
        self.field_name
            .map_or_else(|| self.name.replace('-', "_"), str::to_owned)
    }
}

const REPO: Flag = Flag::new("repo", FlagKind::Value).default(".");
const SCOPE: Flag = Flag::new("scope", FlagKind::Value).default("default");

/// The description of each CLI command group that has one, by its words.
pub const GROUP_DESCRIPTIONS: &[(&str, &str)] = &[
    (
        "report",
        "Deterministic report rendering from a versioned report envelope",
    ),
    (
        "dogfood",
        "Dev-build-only: record pain-point notes about provenance itself",
    ),
    (
        "dogfood triage",
        "Dev-build-only triage of captured notes: mark them handled or reopen them. State \
         lives beside the spool; the capture history is never rewritten",
    ),
];

/// The repository options that the porcelain commands and the commands after them take.
pub const GLOBAL_FLAGS: &[Flag] = &[
    REPO.hyphen_values().global(),
    SCOPE.hyphen_values().global(),
    json_format_flag(None).global(),
    Flag::new("quiet", FlagKind::Switch).global(),
];

/// The repository and scope that a command opens.
pub const REPO_FLAGS: &[Flag] = &[REPO, SCOPE];

/// The `--format` flag of a command that renders only JSON.
pub const fn json_format_flag(default: Option<&'static str>) -> Flag {
    let flag = Flag::new("format", FlagKind::Value).values(&["json"]);
    match default {
        Some(default) => flag.default(default),
        None => flag,
    }
}

/// What one command is on each interface. It holds no handler.
#[derive(Clone, Copy, Debug)]
pub struct Declaration {
    pub name: &'static str,
    /// The description. An empty description shows no about line in the CLI help.
    pub description: &'static str,
    pub cli_form: CliForm,
    pub cli_input: Input,
    pub mcp_input: Input,
    /// The input fields that the CLI reads by position, in order.
    pub positional: &'static [&'static str],
    pub relations: &'static [Relation],
    pub hidden: bool,
    /// The CLI flags, in help order.
    pub flags: &'static [Flag],
}

/// Every command on the CLI and every MCP tool.
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
    /// Dev-build-only commands. The CLI shows them only with the `dogfood` feature.
    Dogfood(DogfoodCommand),
}

impl Command {
    /// Every command, in MCP tool-list order.
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
        Self::Workspace(WorkspaceCommand::DocsServe),
        Self::Workspace(WorkspaceCommand::SkillsList),
        Self::Workspace(WorkspaceCommand::SkillsShow),
        Self::Workspace(WorkspaceCommand::SkillsInstall),
        Self::Workspace(WorkspaceCommand::ReportRender),
        Self::Workspace(WorkspaceCommand::ReportBuild),
        Self::Workspace(WorkspaceCommand::Review),
        Self::Workspace(WorkspaceCommand::Mcp),
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

    /// Whether `interface` shows this command. Each interface has one exhaustive match,
    /// so a new command does not compile until each interface records it.
    #[rule("rule_porcelain_interface_records_each_command")]
    pub const fn presence(self, interface: Interface) -> Presence {
        match interface {
            Interface::Cli => match self {
                Self::Get | Self::Search | Self::Api | Self::Check | Self::Trace | Self::Prime => {
                    Presence::Present
                }
                Self::Change => Presence::Absent {
                    reason: "the CLI takes each record action as a word after the target",
                },
                Self::Action(action) => match action {
                    Action::Create
                    | Action::Update
                    | Action::Answer
                    | Action::Claim
                    | Action::Release
                    | Action::Submit
                    | Action::Discussions
                    | Action::Discussion
                    | Action::Discuss
                    | Action::Reply => Presence::Present,
                },
                Self::Repository(command) => command.presence(interface),
                Self::Graph(command) => command.presence(interface),
                Self::Workspace(command) => command.presence(interface),
                Self::Dogfood(command) => command.presence(interface),
            },
            Interface::Mcp => match self {
                Self::Get | Self::Search | Self::Api | Self::Check | Self::Trace | Self::Change => {
                    Presence::Present
                }
                Self::Prime => Presence::Absent {
                    reason: "MCP clients receive the same guidance as the initialize instructions",
                },
                Self::Action(action) => match action {
                    Action::Create
                    | Action::Update
                    | Action::Answer
                    | Action::Claim
                    | Action::Release
                    | Action::Submit => Presence::Absent {
                        reason: "MCP takes each record action as the action argument of change",
                    },
                    Action::Discussions | Action::Discussion | Action::Discuss | Action::Reply => {
                        Presence::Present
                    }
                },
                Self::Repository(command) => command.presence(interface),
                Self::Graph(command) => command.presence(interface),
                Self::Workspace(command) => command.presence(interface),
                Self::Dogfood(command) => command.presence(interface),
            },
        }
    }

    /// Whether `interface` shows this command.
    pub const fn is_present(self, interface: Interface) -> bool {
        matches!(self.presence(interface), Presence::Present)
    }

    /// The command that `words` select on `interface`. On the CLI, `words` are the root
    /// words; on MCP, the one tool name. A target action is not found by its words.
    pub fn find(interface: Interface, words: &[&str]) -> Option<Self> {
        Self::ALL.iter().copied().find(|command| {
            if !command.is_present(interface) {
                return false;
            }
            let declaration = command.declaration();
            match interface {
                Interface::Mcp => words == [declaration.name],
                Interface::Cli => match declaration.cli_form {
                    CliForm::Words(selected) | CliForm::WordsAndTarget(selected) => {
                        words == selected
                    }
                    CliForm::Target => false,
                },
            }
        })
    }

    /// The target action with this CLI name.
    pub fn find_target_action(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|command| {
            matches!(
                command.declaration().cli_form,
                CliForm::Target | CliForm::WordsAndTarget(_)
            ) && command.declaration().name == name
        })
    }
}

#[cfg(test)]
mod tests {
    use super::porcelain::VIEWS;
    use super::*;
    use crate::get::View;

    /// Implementation aid: the view values in the flag table are the get views.
    #[test]
    fn the_get_view_flag_lists_every_view() {
        assert_eq!(VIEWS, View::ALL.map(View::as_str));
    }

    /// Implementation aid: the typed CLI inputs have one schema property for each flag.
    #[test]
    fn typed_cli_inputs_have_a_property_for_each_flag() {
        for command in Command::ALL {
            let declaration = command.declaration();
            let Input::Typed(schema) = declaration.cli_input else {
                continue;
            };
            let schema = schema();
            let properties = schema["properties"].as_object().unwrap();
            let mut fields = declaration
                .flags
                .iter()
                .map(Flag::field_id)
                .collect::<Vec<_>>();
            fields.sort();
            let mut names = properties.keys().cloned().collect::<Vec<_>>();
            names.sort();
            assert_eq!(fields, names, "{command:?}");
        }
    }
}
