//! The CLI commands that start a server: review, docs serve, and mcp. They are not
//! porcelain commands, so the CLI declares them here.

use crate::cli_form::{CliDeclaration, CliForm, Flag, FlagKind, FlagValue, REPO, SCOPE};
use provenance_porcelain::registry::Grammar;
use serde::Deserialize;

/// One CLI command that starts a server and runs until it stops.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Server {
    DocsServe,
    Review,
    Mcp,
}

/// Each server command, in help order.
pub const SERVERS: &[Server] = &[Server::DocsServe, Server::Review, Server::Mcp];

const DOCS_SERVE_FLAGS: &[Flag] = &[
    REPO,
    Flag::new("host", FlagKind::Value).default("127.0.0.1"),
    Flag::new("port", FlagKind::Value)
        .value(FlagValue::Port)
        .default("5174"),
];

const REVIEW_FLAGS: &[Flag] = &[
    Flag::new("repo", FlagKind::Value)
        .required()
        .help("Repository with an initialized Provenance manifest"),
    Flag::new("repository-id", FlagKind::Value)
        .required()
        .help("Opaque repository target exposed to the browser"),
    Flag::new("scope", FlagKind::Value).required(),
    Flag::new("port", FlagKind::Value)
        .value(FlagValue::Port)
        .default("0")
        .help("Loopback port. Zero selects an available port"),
    Flag::new("no-open", FlagKind::Switch).help("Do not open the review page in a browser"),
    Flag::new("detach", FlagKind::Switch).help(
        "Reuse the running host for this repository and scope, or start one in the background \
         that keeps running after this command returns. Prints where the review page is served",
    ),
];

const MCP_FLAGS: &[Flag] = &[REPO, SCOPE];

/// The `docs serve` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocsServeInput {
    pub repo: String,
    pub host: String,
    pub port: u16,
}

/// The `review` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReviewInput {
    pub repo: String,
    pub repository_id: String,
    pub scope: String,
    pub port: u16,
    #[serde(default)]
    pub no_open: bool,
    #[serde(default)]
    pub detach: bool,
}

/// The `mcp` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct McpInput {
    pub repo: String,
    pub scope: String,
}

impl Server {
    /// The declaration of this server command.
    pub const fn declaration(self) -> CliDeclaration {
        let (name, description, flags) = match self {
            Self::DocsServe => ("docs serve", "", DOCS_SERVE_FLAGS),
            Self::Review => (
                "review",
                "Serve local review assets and authorized repository operations",
                REVIEW_FLAGS,
            ),
            Self::Mcp => (
                "mcp",
                "Serve the MCP tools for one repository on standard input and output",
                MCP_FLAGS,
            ),
        };
        CliDeclaration {
            name,
            description,
            grammar: Grammar::Name,
            form: CliForm::new(flags),
        }
    }

    /// The schema of the input that the CLI reads from the flags of this command.
    pub fn input_schema(self) -> serde_json::Value {
        let schema = match self {
            Self::DocsServe => schemars::schema_for!(DocsServeInput),
            Self::Review => schemars::schema_for!(ReviewInput),
            Self::Mcp => schemars::schema_for!(McpInput),
        };
        serde_json::to_value(schema).expect("schema is JSON")
    }

    /// The server command that the CLI root words select.
    pub fn find(words: &[&str]) -> Option<Self> {
        SERVERS.iter().copied().find(|server| {
            server
                .declaration()
                .words()
                .is_some_and(|selected| selected == words)
        })
    }
}
