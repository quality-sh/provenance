//! Declarations of the porcelain commands: get, search, api, check, prime, and the
//! target-first actions.

use super::{CliForm, Command, Declaration, Flag, FlagKind, FlagValue, Input, Relation};
use crate::action::Action;
use crate::check::Category;
use crate::get::View;
use provenance_core::NodeType;
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

pub(super) const VIEWS: &[&str] = &["record", "children", "trace", "impact", "review"];

/// The get flags after the target, in help order.
const GET_FLAGS: &[Flag] = &[
    Flag::new("view", FlagKind::Value).values(VIEWS),
    Flag::new("depth", FlagKind::Value).value(FlagValue::Count),
    Flag::new("kind", FlagKind::Repeated).value(FlagValue::RecordKind),
    Flag::new("limit", FlagKind::Value).value(FlagValue::Count),
    Flag::new("follow", FlagKind::Switch)
        .help("Stream the current review document and pushed changes as JSON lines."),
    Flag::new("review-link", FlagKind::Switch)
        .help("Print a link to this record in the running review host."),
    Flag::new("notify", FlagKind::Value)
        .values(&["codex", "off"])
        .help("Queue review changes into a Codex thread, or end that registration with off."),
    Flag::new("thread", FlagKind::Value)
        .help("The Codex thread UUID for --notify. The default is CODEX_THREAD_ID."),
    Flag::new("no-open", FlagKind::Switch)
        .help("Print the review link and do not open it in a browser."),
];

const GET_RELATIONS: &[Relation] = &[
    Relation::Requires("follow", "view"),
    Relation::Conflicts("follow", "review_link"),
    Relation::Conflicts("review_link", "view"),
    Relation::Conflicts("review_link", "depth"),
    Relation::Conflicts("review_link", "kind"),
    Relation::Conflicts("review_link", "limit"),
    Relation::Requires("notify", "view"),
    Relation::Conflicts("notify", "follow"),
    Relation::Conflicts("notify", "review_link"),
    Relation::Requires("thread", "notify"),
    Relation::Requires("no_open", "review_link"),
];

/// The trace flags after the target: the get traversal bounds.
const TRACE_FLAGS: &[Flag] = &[
    Flag::new("depth", FlagKind::Value).value(FlagValue::Count),
    Flag::new("kind", FlagKind::Repeated).value(FlagValue::RecordKind),
    Flag::new("limit", FlagKind::Value).value(FlagValue::Count),
];

const SEARCH_FLAGS: &[Flag] = &[
    Flag::new("text", FlagKind::Value).hyphen_values(),
    Flag::new("cursor", FlagKind::Value).hyphen_values(),
    Flag::new("kind", FlagKind::Repeated).value(FlagValue::RecordKind),
    Flag::new("limit", FlagKind::Value).value(FlagValue::Count),
    Flag::new("include-terminal", FlagKind::Switch)
        .help("Include archived and other terminal records"),
    Flag::new("file", FlagKind::Value)
        .hyphen_values()
        .help("Return the Rules bound in this repository-relative source file"),
    Flag::new("symbol", FlagKind::Value)
        .hyphen_values()
        .help("Put the Rules bound to this symbol first; needs --file"),
    Flag::new("line", FlagKind::Value)
        .value(FlagValue::Count)
        .help("Put the Rules bound at this line first; needs --file"),
];

const API_FLAGS: &[Flag] = &[
    Flag::new("path", FlagKind::Positional),
    Flag::new("method", FlagKind::Value).value(FlagValue::ApiMethod),
    Flag::new("input", FlagKind::Value).hyphen_values().help(
        "Read the JSON body from this file, or from standard input with \"-\". The body is the \
         data object, without the HTTP {\"data\": ...} wrapper",
    ),
    Flag::new("header", FlagKind::Repeated),
    Flag::new("query", FlagKind::Repeated),
    Flag::new("filter", FlagKind::Value)
        .help("Show only catalog routes whose method, path, or description contains this text"),
    Flag::new("limit", FlagKind::Value)
        .value(FlagValue::CatalogLimit)
        .help("Show at most this many catalog routes. The default is 50"),
];

const CHECK_FLAGS: &[Flag] = &[
    Flag::new("repo", FlagKind::Value).default("."),
    Flag::new("strict", FlagKind::Switch)
        .help("Reject findings in statements committed at Git HEAD, and every bindings finding"),
    Flag::new("base", FlagKind::Value)
        .help("Compare Git HEAD with this commit instead of its first parent"),
    Flag::new("statement", FlagKind::Value)
        .help("Check this statement instead of the committed statements"),
    Flag::new("graph", FlagKind::Switch)
        .help("Run only graph validity checks. Combine with other check selectors"),
    Flag::new("statements", FlagKind::Switch)
        .help("Run only statement quality checks. Combine with other check selectors"),
    Flag::new("bindings", FlagKind::Switch).help(
        "Run only Rule binding checks. The scan covers the repository root; --path limits it",
    ),
    Flag::new("gaps", FlagKind::Switch).help(
        "Run only the gaps check: report the unfinished parts of the graph and each Rule \
         without a Source. These findings do not fail the check",
    ),
    Flag::new("scope", FlagKind::Value)
        .help("Check only this scope. The default is every declared scope"),
    Flag::new("path", FlagKind::Value).help(
        "Bindings check only: scan the files below this path. The default is the repository root",
    ),
    Flag::new("baseline", FlagKind::Value).help(
        "Bindings check only: resolve evidence anchors against this earlier scan report. \
         Needs --scope",
    ),
    Flag::new("output", FlagKind::Value).help(
        "Bindings check only: write the scan report of the selected scope to this file. \
         Needs --scope",
    ),
    Flag::new("output-format", FlagKind::Value).values(&["json", "markdown"]),
    Flag::new("format", FlagKind::Value).values(&["json"]),
];

const PRIME_FLAGS: &[Flag] = &[
    Flag::new("repo", FlagKind::Value)
        .default(".")
        .help("Deprecated: accepted for compatibility; guidance does not use a repository"),
    Flag::new("scope", FlagKind::Value)
        .default("default")
        .help("Deprecated: accepted for compatibility; guidance does not use a scope"),
    Flag::new("format", FlagKind::Value)
        .values(&["json", "markdown"])
        .default("markdown"),
    Flag::new("include-threads", FlagKind::Switch)
        .help("Deprecated: accepted for compatibility; guidance does not include project threads"),
];

const CREATE_FLAGS: &[Flag] = &[
    Flag::new("type", FlagKind::Value)
        .field("record_type")
        .value(FlagValue::RecordKind),
    Flag::new("stdin", FlagKind::Switch),
];

const RECORD_FLAGS: &[Flag] = &[Flag::new("stdin", FlagKind::Switch)];

const UPDATE_FLAGS: &[Flag] = &[
    Flag::new("stdin", FlagKind::Switch),
    Flag::new("replace-parent", FlagKind::Value)
        .help("Replace this Requirement parent and preserve its other parents"),
    Flag::new("with-parent", FlagKind::Value)
        .help("Add this parent before the selected parent is removed"),
];

const UPDATE_RELATIONS: &[Relation] = &[
    Relation::Requires("replace_parent", "with_parent"),
    Relation::Requires("with_parent", "replace_parent"),
];

/// The CLI get input after the target: the view and its bounds.
#[derive(Clone, Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetCliInput {
    pub view: Option<View>,
    pub depth: Option<usize>,
    #[serde(default)]
    pub kind: Vec<NodeType>,
    pub limit: Option<usize>,
    /// Stream the current review document and pushed changes as JSON lines.
    #[serde(default)]
    pub follow: bool,
    /// Print a link to this record in the running review host.
    #[serde(default)]
    pub review_link: bool,
    /// Queue review changes into a Codex thread, or end that registration with off.
    pub notify: Option<String>,
    /// The Codex thread UUID for --notify. The default is `CODEX_THREAD_ID`.
    pub thread: Option<String>,
    /// Print the review link and do not open it in a browser.
    #[serde(default)]
    pub no_open: bool,
}

/// The one format of a command without a human renderer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckFormat {
    Json,
}

/// The CLI check input. Each bool is one CLI switch.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckCliInput {
    pub repo: String,
    /// Reject findings in statements committed at Git HEAD, and every bindings finding.
    #[serde(default)]
    pub strict: bool,
    /// Compare Git HEAD with this commit instead of its first parent.
    pub base: Option<String>,
    /// Check this statement instead of the committed statements.
    pub statement: Option<String>,
    /// Run only graph validity checks. Combine with other check selectors.
    #[serde(default)]
    pub graph: bool,
    /// Run only statement quality checks. Combine with other check selectors.
    #[serde(default)]
    pub statements: bool,
    /// Run only Rule binding checks. The scan covers the repository root; `path` limits it.
    #[serde(default)]
    pub bindings: bool,
    /// Run only the gaps check: the unfinished parts of the graph and each Rule without a Source.
    #[serde(default)]
    pub gaps: bool,
    /// Check only this scope. The default is every declared scope.
    pub scope: Option<String>,
    /// Bindings check only: scan the files below this path.
    pub path: Option<String>,
    /// Bindings check only: an earlier scan report. Needs `--scope`.
    pub baseline: Option<String>,
    /// Bindings check only: the file for the scan report of the selected scope.
    pub output: Option<String>,
    pub output_format: Option<ReportFileFormat>,
    pub format: Option<CheckFormat>,
}

/// The format of the bindings scan report file.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReportFileFormat {
    #[default]
    Json,
    Markdown,
}

impl CheckCliInput {
    /// The categories that the selector flags name, in category order.
    pub fn categories(&self) -> Vec<Category> {
        Category::ALL
            .into_iter()
            .filter(|category| match category {
                Category::Graph => self.graph,
                Category::Statements => self.statements,
                Category::Bindings => self.bindings,
                Category::Gaps => self.gaps,
            })
            .collect()
    }
}

/// The format of the prime guidance.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PrimeFormat {
    Json,
    Markdown,
}

/// The CLI prime input. Guidance does not use a repository, a scope, or threads; the
/// flags stay for compatibility.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrimeInput {
    pub repo: String,
    pub scope: String,
    pub format: PrimeFormat,
    #[serde(default)]
    pub include_threads: bool,
}

fn get_cli_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(GetCliInput)).expect("schema is JSON")
}

fn check_cli_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(CheckCliInput)).expect("schema is JSON")
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
            cli_form: CliForm::Words(&["search"]),
            cli_input: Input::None,
            mcp_input: Input::Typed(crate::search::input_schema),
            positional: &[],
            relations: &[],
            hidden: false,
            flags: SEARCH_FLAGS,
        },
        Command::Api => Declaration {
            name: "api",
            description: crate::api::API_DESCRIPTION,
            cli_form: CliForm::Words(&["api"]),
            cli_input: Input::None,
            mcp_input: Input::Typed(crate::api::input_schema),
            positional: &["path"],
            relations: &[],
            hidden: false,
            flags: API_FLAGS,
        },
        Command::Check => Declaration {
            name: "check",
            description: CHECK_DESCRIPTION,
            cli_form: CliForm::Words(&["check"]),
            cli_input: Input::Typed(check_cli_schema),
            mcp_input: Input::Typed(crate::check::input_schema),
            positional: &[],
            relations: &[
                Relation::Requires("base", "strict"),
                Relation::Conflicts("statement", "base"),
                Relation::Requires("baseline", "scope"),
                Relation::Requires("output", "scope"),
                Relation::Requires("output_format", "output"),
            ],
            hidden: false,
            flags: CHECK_FLAGS,
        },
        Command::Trace => Declaration {
            name: "trace",
            description: TRACE_DESCRIPTION,
            cli_form: CliForm::Target,
            cli_input: Input::None,
            mcp_input: Input::Typed(crate::get::trace_input_schema),
            positional: &[],
            relations: &[],
            hidden: false,
            flags: TRACE_FLAGS,
        },
        Command::Change => Declaration {
            name: "change",
            description: CHANGE_DESCRIPTION,
            cli_form: CliForm::Words(&["change"]),
            cli_input: Input::None,
            mcp_input: Input::None,
            positional: &[],
            relations: &[],
            hidden: false,
            flags: &[],
        },
        Command::Prime => Declaration {
            name: "prime",
            description: PRIME_DESCRIPTION,
            cli_form: CliForm::Words(&["prime"]),
            cli_input: Input::Typed(prime_schema),
            mcp_input: Input::None,
            positional: &[],
            relations: &[],
            hidden: false,
            flags: PRIME_FLAGS,
        },
        Command::Action(action) => action_declaration(action),
        Command::Repository(command) => super::repository::declaration(command),
        Command::Graph(command) => super::graph::declaration(command),
        Command::Workspace(command) => super::workspace::declaration(command),
        Command::Dogfood(command) => super::dogfood::declaration(command),
    }
}

/// The get declaration. Its description names the direction of each view that walks
/// the graph, on the CLI help and the MCP tool.
#[provenance_macros::rule("rule_get_help_names_view_direction")]
const fn get_declaration() -> Declaration {
    Declaration {
        name: "get",
        description: GET_DESCRIPTION,
        cli_form: CliForm::Target,
        cli_input: Input::Typed(get_cli_schema),
        mcp_input: Input::Typed(crate::get::input_schema),
        positional: &[],
        relations: GET_RELATIONS,
        hidden: false,
        flags: GET_FLAGS,
    }
}

const fn action_declaration(action: Action) -> Declaration {
    let (description, flags, relations, input, cli_form): (_, _, &[Relation], _, _) = match action {
        Action::Create => (
            "Create the target ID as an explicit record type.",
            CREATE_FLAGS,
            &[],
            Input::RecordFields(action),
            CliForm::Target,
        ),
        Action::Update => (
            "Update the existing target while preserving omitted fields.",
            UPDATE_FLAGS,
            UPDATE_RELATIONS,
            Input::RecordFields(action),
            CliForm::Target,
        ),
        Action::Answer => (
            "Answer the target Question.",
            RECORD_FLAGS,
            &[],
            Input::RecordFields(action),
            CliForm::Target,
        ),
        Action::Claim => (
            "Claim the target Topic.",
            RECORD_FLAGS,
            &[],
            Input::RecordFields(action),
            CliForm::Target,
        ),
        Action::Release => (
            "Release the target Topic claim.",
            RECORD_FLAGS,
            &[],
            Input::RecordFields(action),
            CliForm::Target,
        ),
        Action::Submit => (
            "Submit the target Requirement for review.",
            RECORD_FLAGS,
            &[],
            Input::RecordFields(action),
            CliForm::Target,
        ),
        Action::Discussions => (
            "List addressed Discussions in a scope or under a record. On MCP, the action \
             argument also reads one Discussion (discussion), and the change tool starts one \
             (discuss) or replies to one (reply).",
            &[],
            &[],
            Input::Discussion(action),
            CliForm::WordsAndTarget(&["discussions"]),
        ),
        Action::Discussion => (
            "Read the addressed Discussion.",
            &[],
            &[],
            Input::Discussion(action),
            CliForm::Target,
        ),
        Action::Discuss => (
            "Start a Discussion under the target record.",
            &[],
            &[],
            Input::Discussion(action),
            CliForm::Target,
        ),
        Action::Reply => (
            "Reply to the addressed Discussion.",
            &[],
            &[],
            Input::Discussion(action),
            CliForm::Target,
        ),
    };
    Declaration {
        name: action.as_str(),
        description,
        cli_form,
        cli_input: input,
        mcp_input: if matches!(action, Action::Discussions) {
            Input::Typed(crate::discussion::tool_input_schema)
        } else {
            input
        },
        positional: &[],
        relations,
        hidden: false,
        flags,
    }
}
