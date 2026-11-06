//! Declarations of the commands that pin graph references and land swarm backtraces.

use super::{json_format, Declaration, Field, Fields, Relation, REPO, SCOPE};
use serde::Deserialize;

/// One graph command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphCommand {
    ReferenceIssue,
    ReferenceShow,
    ReferenceVerify,
    ReferenceExactExport,
    SwarmBacktraceLand,
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

schema_fn!(reference_issue_schema, ReferenceIssueInput);
schema_fn!(reference_schema, ReferenceInput);
schema_fn!(swarm_backtrace_land_schema, SwarmBacktraceLandInput);

const JSON_FORMAT: Field = json_format(Some("json"));

const REFERENCE_ISSUE_FIELDS: &[Field] = &[
    REPO,
    SCOPE,
    Field::text("commit")
        .doc("Git revision to pin. When omitted, clean relevant state at HEAD is required"),
    Field::text("correlation_system"),
    Field::text("correlation_key"),
];

const REFERENCE_FIELDS: &[Field] = &[REPO, Field::text("reference").required()];

const SWARM_BACKTRACE_LAND_FIELDS: &[Field] = &[
    REPO,
    SCOPE,
    Field::text("run_dir").required(),
    Field::switch("replace"),
    JSON_FORMAT,
];

pub(super) const fn declaration(command: GraphCommand) -> Declaration {
    match command {
        GraphCommand::ReferenceIssue => Declaration::named(
            "graph-reference issue",
            "Issue an immutable reference after canonical graph state is committed",
            Fields::new(REFERENCE_ISSUE_FIELDS, reference_issue_schema).relations(&[
                Relation::Requires("correlation_system", "correlation_key"),
                Relation::Requires("correlation_key", "correlation_system"),
            ]),
        ),
        GraphCommand::ReferenceShow => Declaration::named(
            "graph-reference show",
            "Show metadata and graph-family counts from a pinned reference",
            Fields::new(REFERENCE_FIELDS, reference_schema),
        ),
        GraphCommand::ReferenceVerify => Declaration::named(
            "graph-reference verify",
            "Verify identity and graph content at the pinned Git revision",
            Fields::new(REFERENCE_FIELDS, reference_schema),
        ),
        GraphCommand::ReferenceExactExport => Declaration::named(
            "graph-reference exact-export",
            "Export the canonical graph reconstructed from the pinned Git revision",
            Fields::new(REFERENCE_FIELDS, reference_schema),
        ),
        GraphCommand::SwarmBacktraceLand => Declaration::named(
            "swarm-backtrace land",
            "",
            Fields::new(SWARM_BACKTRACE_LAND_FIELDS, swarm_backtrace_land_schema),
        ),
    }
}
