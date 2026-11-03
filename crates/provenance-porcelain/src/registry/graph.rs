//! Declarations of the CLI-only commands that read the graph, scan bindings, pin graph
//! references, and land swarm backtraces.

use super::{
    json_format_flag, CliForm, Declaration, Flag, FlagKind, Input, Interface, Presence, Relation,
    REPO, SCOPE,
};
use serde::Deserialize;

/// One CLI-only graph command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphCommand {
    Graph,
    Traceability,
    Gaps,
    Health,
    Orphans,
    CoverageScan,
    ReferenceIssue,
    ReferenceShow,
    ReferenceVerify,
    ReferenceExactExport,
    SwarmBacktraceLand,
}

impl GraphCommand {
    /// Whether `interface` shows this command. Each interface has one exhaustive match.
    pub(super) const fn presence(self, interface: Interface) -> Presence {
        match interface {
            Interface::Cli => match self {
                Self::Graph
                | Self::Traceability
                | Self::Gaps
                | Self::Health
                | Self::Orphans
                | Self::CoverageScan
                | Self::ReferenceIssue
                | Self::ReferenceShow
                | Self::ReferenceVerify
                | Self::ReferenceExactExport
                | Self::SwarmBacktraceLand => Presence::Present,
            },
            Interface::Mcp => match self {
                Self::Graph
                | Self::Traceability
                | Self::Gaps
                | Self::Health
                | Self::Orphans
                | Self::CoverageScan
                | Self::ReferenceIssue
                | Self::ReferenceShow
                | Self::ReferenceVerify
                | Self::ReferenceExactExport
                | Self::SwarmBacktraceLand => Presence::Absent {
                    reason: "MCP clients read the graph through get, search, and api",
                },
            },
        }
    }
}

const JSON_FORMAT: Flag = json_format_flag(Some("json"));

const GRAPH_FLAGS: &[Flag] = &[
    Flag::new("requirement-id", FlagKind::Positional).required(),
    REPO,
    SCOPE,
    JSON_FORMAT,
];

const TRACEABILITY_FLAGS: &[Flag] = &[
    Flag::new("rule-id", FlagKind::Positional).required(),
    REPO,
    SCOPE,
    JSON_FORMAT,
];

const READ_FLAGS: &[Flag] = &[REPO, SCOPE, JSON_FORMAT];

const COVERAGE_SCAN_FLAGS: &[Flag] = &[
    REPO,
    Flag::new("path", FlagKind::Value).required(),
    SCOPE,
    Flag::new("baseline", FlagKind::Value).help(
        "JSON report from an earlier scan whose evidence anchors should be resolved against \
         this scan",
    ),
    Flag::new("validate-rules", FlagKind::Switch),
    Flag::new("strict", FlagKind::Switch).help(
        "Exit non-zero when the report contains any warnings. The report is still printed \
         first",
    ),
    Flag::new("format", FlagKind::Value)
        .values(&["json", "markdown"])
        .default("markdown"),
    Flag::new("output", FlagKind::Value),
];

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

/// The `graph` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RequirementGraphInput {
    pub requirement_id: String,
    pub repo: String,
    pub scope: String,
    pub format: String,
}

/// The `traceability` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TraceabilityInput {
    pub rule_id: String,
    pub repo: String,
    pub scope: String,
    pub format: String,
}

/// The `gaps`, `health`, and `orphans` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphReadInput {
    pub repo: String,
    pub scope: String,
    pub format: String,
}

/// The `coverage scan` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoverageScanInput {
    pub repo: String,
    pub path: String,
    pub scope: String,
    pub baseline: Option<String>,
    #[serde(default)]
    pub validate_rules: bool,
    #[serde(default)]
    pub strict: bool,
    pub format: String,
    pub output: Option<String>,
}

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

schema_fn!(requirement_graph_schema, RequirementGraphInput);
schema_fn!(traceability_schema, TraceabilityInput);
schema_fn!(graph_read_schema, GraphReadInput);
schema_fn!(coverage_scan_schema, CoverageScanInput);
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
        GraphCommand::Graph => Declaration {
            positional: &["requirement_id"],
            ..cli_only(&["graph"], "", requirement_graph_schema, GRAPH_FLAGS)
        },
        GraphCommand::Traceability => Declaration {
            positional: &["rule_id"],
            ..cli_only(
                &["traceability"],
                "",
                traceability_schema,
                TRACEABILITY_FLAGS,
            )
        },
        GraphCommand::Gaps => cli_only(&["gaps"], "", graph_read_schema, READ_FLAGS),
        GraphCommand::Health => cli_only(&["health"], "", graph_read_schema, READ_FLAGS),
        GraphCommand::Orphans => cli_only(&["orphans"], "", graph_read_schema, READ_FLAGS),
        GraphCommand::CoverageScan => Declaration {
            name: "coverage scan",
            ..cli_only(
                &["coverage", "scan"],
                "",
                coverage_scan_schema,
                COVERAGE_SCAN_FLAGS,
            )
        },
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
