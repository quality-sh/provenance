//! Declarations of the commands that pin graph references and land swarm backtraces.

use super::Declaration;
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

pub(super) const fn declaration(command: GraphCommand) -> Declaration {
    match command {
        GraphCommand::ReferenceIssue => Declaration::named(
            "graph-reference issue",
            "Issue an immutable reference after canonical graph state is committed",
            reference_issue_schema,
        ),
        GraphCommand::ReferenceShow => Declaration::named(
            "graph-reference show",
            "Show metadata and graph-family counts from a pinned reference",
            reference_schema,
        ),
        GraphCommand::ReferenceVerify => Declaration::named(
            "graph-reference verify",
            "Verify identity and graph content at the pinned Git revision",
            reference_schema,
        ),
        GraphCommand::ReferenceExactExport => Declaration::named(
            "graph-reference exact-export",
            "Export the canonical graph reconstructed from the pinned Git revision",
            reference_schema,
        ),
        GraphCommand::SwarmBacktraceLand => {
            Declaration::named("swarm-backtrace land", "", swarm_backtrace_land_schema)
        }
    }
}
