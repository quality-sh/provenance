//! The CLI grammar of each command: its flags, positionals, flag relations, and whether
//! help shows it. Porcelain declares the name, description, target grammar, and input.

mod dogfood;
mod flags;
mod graph;
mod porcelain;
mod repository;
mod workspace;

pub use flags::{
    json_format_flag, Flag, FlagKind, FlagValue, Relation, GLOBAL_FLAGS, GROUP_DESCRIPTIONS, REPO,
    SCOPE,
};
pub use porcelain::{CheckCliInput, CheckFormat, GetCliInput, ReportFileFormat};
use provenance_porcelain::registry::{Command, Grammar, Input};

/// The CLI grammar of one command after its words or target.
#[derive(Clone, Copy, Debug)]
pub struct CliForm {
    /// The input fields that the CLI reads by position, in order.
    pub positional: &'static [&'static str],
    pub relations: &'static [Relation],
    pub hidden: bool,
    /// The CLI flags, in help order.
    pub flags: &'static [Flag],
}

impl CliForm {
    /// A shown form with these flags, no positionals, and no relations.
    pub const fn new(flags: &'static [Flag]) -> Self {
        Self {
            positional: &[],
            relations: &[],
            hidden: false,
            flags,
        }
    }

    #[must_use]
    pub const fn positional(mut self, positional: &'static [&'static str]) -> Self {
        self.positional = positional;
        self
    }

    #[must_use]
    pub const fn relations(mut self, relations: &'static [Relation]) -> Self {
        self.relations = relations;
        self
    }

    #[must_use]
    pub const fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }
}

/// One CLI command: its name, description, and target grammar, with its CLI form.
#[derive(Clone, Copy, Debug)]
pub struct CliDeclaration {
    pub name: &'static str,
    /// The description. An empty description shows no about line in the CLI help.
    pub description: &'static str,
    pub grammar: Grammar,
    pub form: CliForm,
}

impl CliDeclaration {
    /// The words of `provenance <words...>`, when the name selects the command.
    pub fn words(&self) -> Option<Vec<&'static str>> {
        self.grammar
            .takes_name()
            .then(|| self.name.split(' ').collect())
    }
}

/// The CLI grammar of one porcelain command.
pub const fn form(command: Command) -> CliForm {
    match command {
        Command::Repository(command) => repository::form(command),
        Command::Graph(command) => graph::form(command),
        Command::Workspace(command) => workspace::form(command),
        Command::Dogfood(command) => dogfood::form(command),
        _ => porcelain::form(command),
    }
}

/// One porcelain command as the CLI shows it.
pub fn declaration(command: Command) -> CliDeclaration {
    let declaration = command.declaration();
    CliDeclaration {
        name: declaration.name,
        description: declaration.description,
        grammar: declaration.grammar,
        form: form(command),
    }
}

/// The schema of the typed input that the CLI reads from the flags of `command`, when
/// the CLI reads one.
pub fn input_schema(command: Command) -> Option<serde_json::Value> {
    match command {
        Command::Get => Some(porcelain::get_cli_schema()),
        Command::Check => Some(porcelain::check_cli_schema()),
        Command::Search | Command::Api | Command::Trace | Command::Action(_) => None,
        _ => match command.declaration().input {
            Input::Typed(schema) => Some(schema()),
            Input::RecordFields(_) | Input::Discussion(_) | Input::None => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Implementation aid: the typed CLI inputs have one schema property for each flag.
    #[test]
    fn typed_cli_inputs_have_a_property_for_each_flag() {
        for command in Command::ALL {
            let Some(schema) = input_schema(*command) else {
                continue;
            };
            let properties = schema["properties"].as_object().unwrap();
            let mut fields = form(*command)
                .flags
                .iter()
                .map(Flag::field_id)
                .collect::<Vec<_>>();
            fields.sort();
            let mut names = properties.keys().cloned().collect::<Vec<_>>();
            names.sort();
            assert_eq!(fields, names, "{command:?}");
        }
        for server in crate::servers::SERVERS {
            let schema = server.input_schema();
            let mut fields = server
                .declaration()
                .form
                .flags
                .iter()
                .map(Flag::field_id)
                .collect::<Vec<_>>();
            fields.sort();
            let mut names = schema["properties"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            names.sort();
            assert_eq!(fields, names, "{server:?}");
        }
    }
}
