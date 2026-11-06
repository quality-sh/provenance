//! Declarations of the record commands: get, search, api, check, prime, and the
//! target actions.

use super::{Command, Declaration, Grammar, Input};
use crate::action::Action;
use serde::Deserialize;

const GET_DESCRIPTION: &str = "Read one repository record by its repository-local ID. Select \
     its record view, children view (records below this record), trace view (records above this \
     record, up to its Sources), impact view, or review view.";
const TRACE_DESCRIPTION: &str =
    "Read the records above this record, up to its Sources. The get trace view gives the same records.";
const SEARCH_DESCRIPTION: &str = "Find records across the permitted kinds in the bound scope, or the Rules bound to a source file and symbol.";
const CHECK_DESCRIPTION: &str = "Check graph validity, statement quality, and binding \
     coverage. Give a statement to check that text with the project dictionary before a write.";
const CHANGE_DESCRIPTION: &str = "Write to the graph. The action argument selects the \
     write: create, update, answer, claim, release, or submit on the target record; discuss or \
     reply for a Discussion; or notify to register a client thread for review changes. Omit \
     data to get the data schema of the action and record kind.";
const PRIME_DESCRIPTION: &str = "Introduce Provenance domain concepts and actions.";

/// The format of the prime guidance.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrimeFormat {
    Json,
    Markdown,
}

/// The prime input. Guidance does not use a repository, a scope, or threads; these
/// fields stay for compatibility.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrimeInput {
    pub repo: String,
    pub scope: String,
    pub format: PrimeFormat,
    #[serde(default)]
    pub include_threads: bool,
}

fn prime_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(PrimeInput)).expect("schema is JSON")
}

pub(super) fn declaration(command: Command) -> Declaration {
    match command {
        Command::Get => get_declaration(),
        Command::Search => Declaration {
            name: "search",
            description: SEARCH_DESCRIPTION,
            grammar: Grammar::Name,
            input: Input::Typed(crate::search::input_schema),
        },
        Command::Api => Declaration {
            name: "api",
            description: crate::api::API_DESCRIPTION,
            grammar: Grammar::Name,
            input: Input::Typed(crate::api::input_schema),
        },
        Command::Check => Declaration {
            name: "check",
            description: CHECK_DESCRIPTION,
            grammar: Grammar::Name,
            input: Input::Typed(crate::check::input_schema),
        },
        Command::Trace => Declaration {
            name: "trace",
            description: TRACE_DESCRIPTION,
            grammar: Grammar::Target,
            input: Input::Typed(crate::get::trace_input_schema),
        },
        Command::Change => Declaration {
            name: "change",
            description: CHANGE_DESCRIPTION,
            grammar: Grammar::Name,
            input: Input::None,
        },
        Command::Prime => Declaration {
            name: "prime",
            description: PRIME_DESCRIPTION,
            grammar: Grammar::Name,
            input: Input::Typed(prime_schema),
        },
        Command::Action(action) => action_declaration(action),
        Command::Repository(command) => super::repository::declaration(command),
        Command::Graph(command) => super::graph::declaration(command),
        Command::Workspace(command) => super::workspace::declaration(command),
        Command::Dogfood(command) => super::dogfood::declaration(command),
    }
}

/// The get declaration. Its description names the direction of each view that walks
/// the graph, wherever a consumer shows it.
#[provenance_macros::rule("rule_get_help_names_view_direction")]
const fn get_declaration() -> Declaration {
    Declaration {
        name: "get",
        description: GET_DESCRIPTION,
        grammar: Grammar::Target,
        input: Input::Typed(crate::get::input_schema),
    }
}

const fn action_declaration(action: Action) -> Declaration {
    let (description, grammar) = match action {
        Action::Create => (
            "Create the target ID as an explicit record type.",
            Grammar::Target,
        ),
        Action::Update => (
            "Update the existing target while preserving omitted fields.",
            Grammar::Target,
        ),
        Action::Answer => ("Answer the target Question.", Grammar::Target),
        Action::Claim => ("Claim the target Topic.", Grammar::Target),
        Action::Release => ("Release the target Topic claim.", Grammar::Target),
        Action::Submit => ("Submit the target Requirement for review.", Grammar::Target),
        Action::Discussions => (
            "List addressed Discussions in a scope or under a record. On MCP, the action \
             argument also reads one Discussion (discussion), and the change tool starts one \
             (discuss) or replies to one (reply).",
            Grammar::NameAndTarget,
        ),
        Action::Discussion => ("Read the addressed Discussion.", Grammar::Target),
        Action::Discuss => (
            "Start a Discussion under the target record.",
            Grammar::Target,
        ),
        Action::Reply => ("Reply to the addressed Discussion.", Grammar::Target),
    };
    let input = match action {
        // The Discussions input lists Discussions and reads one.
        Action::Discussions => Input::Typed(crate::discussion::tool_input_schema),
        Action::Discussion | Action::Discuss | Action::Reply => Input::Discussion(action),
        _ => Input::RecordFields(action),
    };
    Declaration {
        name: action.as_str(),
        description,
        grammar,
        input,
    }
}
