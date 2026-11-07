//! The `provenance mcp` command: the MCP server for one repository on standard input and output.

use provenance_cli::servers::McpInput;
use provenance_transport::repository::RepoContext;

/// Serves the MCP tools for the repository and scope that the CLI commands open, until
/// standard input closes, then lets started work finish.
#[provenance_macros::rule("rule_mcp_command_uses_cli_repository")]
pub(super) async fn handle(input: McpInput) -> anyhow::Result<()> {
    let context = RepoContext::new(input.repo, input.scope);
    let host = provenance_cli::porcelain::local_host(&context)?;
    let session = provenance_mcp::serve_stdio(host.clone())
        .await
        .map_err(|error| anyhow::anyhow!("cannot start the MCP session: {error}"))?;
    let ended = session.waiting().await;
    host.shutdown().await;
    ended.map_err(|error| anyhow::anyhow!("the MCP session stopped: {error}"))
}
