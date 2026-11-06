//! The CLI grammar of get, search, api, check, prime, and the target actions.

use super::{CliForm, Flag, FlagKind, FlagValue, Relation};
use provenance_core::NodeType;
use provenance_porcelain::action::Action;
use provenance_porcelain::check::Category;
use provenance_porcelain::get::View;
use provenance_porcelain::registry::Command;
use serde::Deserialize;

const VIEWS: &[&str] = &["record", "children", "trace", "impact", "review"];

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
    Flag::new("docs", FlagKind::Switch).help(
        "Run only the docs check: report each Markdown link that names a page the docs site \
         does not publish",
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
    /// Run only the docs check: each Markdown link that names a page the docs site does not publish.
    #[serde(default)]
    pub docs: bool,
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
                Category::Docs => self.docs,
            })
            .collect()
    }
}

const CHECK_RELATIONS: &[Relation] = &[
    Relation::Requires("base", "strict"),
    Relation::Conflicts("statement", "base"),
    Relation::Requires("baseline", "scope"),
    Relation::Requires("output", "scope"),
    Relation::Requires("output_format", "output"),
];

pub(super) fn get_cli_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(GetCliInput)).expect("schema is JSON")
}

pub(super) fn check_cli_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(CheckCliInput)).expect("schema is JSON")
}

pub(super) const fn form(command: Command) -> CliForm {
    match command {
        Command::Get => CliForm::new(GET_FLAGS).relations(GET_RELATIONS),
        Command::Search => CliForm::new(SEARCH_FLAGS),
        Command::Api => CliForm::new(API_FLAGS).positional(&["path"]),
        Command::Check => CliForm::new(CHECK_FLAGS).relations(CHECK_RELATIONS),
        Command::Trace => CliForm::new(TRACE_FLAGS),
        Command::Prime => CliForm::new(PRIME_FLAGS),
        Command::Action(action) => action_form(action),
        Command::Change
        | Command::Repository(_)
        | Command::Graph(_)
        | Command::Workspace(_)
        | Command::Dogfood(_) => CliForm::new(&[]),
    }
}

const fn action_form(action: Action) -> CliForm {
    match action {
        Action::Create => CliForm::new(CREATE_FLAGS),
        Action::Update => CliForm::new(UPDATE_FLAGS).relations(UPDATE_RELATIONS),
        Action::Answer | Action::Claim | Action::Release | Action::Submit => {
            CliForm::new(RECORD_FLAGS)
        }
        Action::Discussions | Action::Discussion | Action::Discuss | Action::Reply => {
            CliForm::new(&[])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Implementation aid: the view values in the flag table are the get views.
    #[test]
    fn the_get_view_flag_lists_every_view() {
        assert_eq!(VIEWS, View::ALL.map(View::as_str));
    }
}
