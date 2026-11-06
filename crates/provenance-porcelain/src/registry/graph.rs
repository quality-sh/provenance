//! Declarations of the CLI-only commands that pin graph
//! references, and land swarm backtraces.

use super::{json_format_flag, CliForm, Declaration, Flag, FlagKind, Input, Relation, REPO, SCOPE};
use serde::Deserialize;

/// One CLI-only graph command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphCommand {
    ReferenceIssue,
    ReferenceShow,
    ReferenceVerify,
    ReferenceExactExport,
    SwarmBacktraceLand,
}

const JSON_FORMAT: Flag = json_format_flag(Some("json"));

const REFERENCE_ISSUE_FLAGS: &[Flag] = &[
    REPO,
    SCOPE,
    Flag::new("commit", FlagKind::Value)
        .help("Git revision to pin. When omitted, clean relevant state at HEAD is required"),
    Flag::new("correlation-system", FlagKind::Value),
    Flag::new("correlation-key", FlagKind::Value),
];

const REFERENCE_FLAGS: &[Flag] = &[REPO, Flag::new("reference", FlagKind::Value).required()];

const SWARM_BACKTRACE_LAND_FLAGS: &[Flag] = &[
    REPO,
    SCOPE,
    Flag::new("run-dir", FlagKind::Value).required(),
    Flag::new("replace", FlagKind::Switch),
    JSON_FORMAT,
];

/// The `graph-reference issue` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReferenceIssueInput {
    pub repo: String,
    pub scope: String,
    pub commit: Option<String>,
    pub correlation_system: Option<String>,
    pub correlation_key: Option<String>,
}

/// The `graph-reference show`, `verify`, and `exact-export` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReferenceInput {
    pub repo: String,
    pub reference: String,
}

/// The `swarm-backtrace land` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SwarmBacktraceLandInput {
    pub repo: String,
    pub scope: String,
    pub run_dir: String,
    #[serde(default)]
    pub replace: bool,
    pub format: String,
}

macro_rules! schema_fn {
    ($name:ident, $input:ty) => {
        fn $name() -> serde_json::Value {
            serde_json::to_value(schemars::schema_for!($input)).expect("schema is JSON")
        }
    };
}

schema_fn!(reference_issue_schema, ReferenceIssueInput);
schema_fn!(reference_schema, ReferenceInput);
schema_fn!(swarm_backtrace_land_schema, SwarmBacktraceLandInput);

/// A shown CLI-only command without flag relations.
const fn cli_only(
    words: &'static [&'static str],
    description: &'static str,
    schema: fn() -> serde_json::Value,
    flags: &'static [Flag],
) -> Declaration {
    Declaration {
        name: words[words.len() - 1],
        description,
        cli_form: CliForm::Words(words),
        cli_input: Input::Typed(schema),
        mcp_input: Input::None,
        positional: &[],
        relations: &[],
        hidden: false,
        flags,
    }
}

pub(super) fn declaration(command: GraphCommand) -> Declaration {
    match command {
        GraphCommand::ReferenceIssue => Declaration {
            name: "graph-reference issue",
            relations: &[
                Relation::Requires("correlation_system", "correlation_key"),
                Relation::Requires("correlation_key", "correlation_system"),
            ],
            ..cli_only(
                &["graph-reference", "issue"],
                "Issue an immutable reference after canonical graph state is committed",
                reference_issue_schema,
                REFERENCE_ISSUE_FLAGS,
            )
        },
        GraphCommand::ReferenceShow => Declaration {
            name: "graph-reference show",
            ..cli_only(
                &["graph-reference", "show"],
                "Show metadata and graph-family counts from a pinned reference",
                reference_schema,
                REFERENCE_FLAGS,
            )
        },
        GraphCommand::ReferenceVerify => Declaration {
            name: "graph-reference verify",
            ..cli_only(
                &["graph-reference", "verify"],
                "Verify identity and graph content at the pinned Git revision",
                reference_schema,
                REFERENCE_FLAGS,
            )
        },
        GraphCommand::ReferenceExactExport => Declaration {
            name: "graph-reference exact-export",
            ..cli_only(
                &["graph-reference", "exact-export"],
                "Export the canonical graph reconstructed from the pinned Git revision",
                reference_schema,
                REFERENCE_FLAGS,
            )
        },
        GraphCommand::SwarmBacktraceLand => Declaration {
            name: "swarm-backtrace land",
            ..cli_only(
                &["swarm-backtrace", "land"],
                "",
                swarm_backtrace_land_schema,
                SWARM_BACKTRACE_LAND_FLAGS,
            )
        },
    }
}
