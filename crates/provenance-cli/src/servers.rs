//! The CLI commands that start a server: review and mcp. They are not
//! porcelain commands, so the CLI declares them here.

use provenance_porcelain::registry::{Declaration, Field, FieldValue, Fields, REPO, SCOPE};
use serde::Deserialize;

/// One CLI command that starts a server and runs until it stops.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Server {
    Review,
    Mcp,
}

/// Each server command, in help order.
pub const SERVERS: &[Server] = &[Server::Review, Server::Mcp];

const REVIEW_FIELDS: &[Field] = &[
    Field::text("repo")
        .required()
        .doc("Repository with an initialized Provenance manifest"),
    Field::text("repository_id")
        .required()
        .doc("Opaque repository target exposed to the browser"),
    Field::text("scope").required(),
    Field::text("port")
        .of(FieldValue::Port)
        .default("0")
        .doc("Loopback port. Zero selects an available port"),
    Field::switch("no_open").doc("Do not open the review page in a browser"),
    Field::switch("detach").doc(
        "Reuse the running host for this repository and scope, or start one in the background \
         that keeps running after this command returns. Prints where the review page is served",
    ),
];

const MCP_FIELDS: &[Field] = &[REPO, SCOPE];

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
    pub const fn declaration(self) -> Declaration {
        let (name, description, fields, schema): (_, _, _, fn() -> serde_json::Value) = match self {
            Self::Review => (
                "review",
                "Serve local review assets and authorized repository operations",
                REVIEW_FIELDS,
                review_schema,
            ),
            Self::Mcp => (
                "mcp",
                "Serve the MCP tools for one repository on standard input and output",
                MCP_FIELDS,
                mcp_schema,
            ),
        };
        Declaration::named(name, description, Fields::new(fields, schema))
    }

    /// The server command that the CLI root words select.
    pub fn find(words: &[&str]) -> Option<Self> {
        SERVERS
            .iter()
            .copied()
            .find(|server| server.declaration().words().eq(words.iter().copied()))
    }
}

fn review_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(ReviewInput)).expect("schema is JSON")
}

fn mcp_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(McpInput)).expect("schema is JSON")
}
