//! The `provenance mcp` command: the MCP server for one repository on standard input and output.

use crate::handlers::check::RepositoryCheckPort;
use camino::Utf8PathBuf;
use provenance_cli::repo_context::RepoContext;
use provenance_porcelain::registry::McpInput;
use std::sync::Arc;

/// Serves the MCP tools for the repository and scope that the CLI commands open, until
/// standard input closes, then lets started work finish.
#[provenance_macros::rule("rule_mcp_command_uses_cli_repository")]
pub(super) async fn handle(input: McpInput) -> anyhow::Result<()> {
    let context = RepoContext::new(input.repo, input.scope);
    // The check port reads the canonical working tree, as the opened repository does.
    let working_tree = dunce::canonicalize(&context.repo)
        .ok()
        .and_then(|path| Utf8PathBuf::from_path_buf(path).ok())
        .unwrap_or_else(|| context.repo.clone());
    let host = context
        .local_host()?
        .with_check_port(Arc::new(RepositoryCheckPort::new(
            working_tree,
            false,
            None,
        )));
    let session = provenance_mcp::serve_stdio(host.clone())
        .await
        .map_err(|error| anyhow::anyhow!("cannot start the MCP session: {error}"))?;
    let ended = session.waiting().await;
    host.shutdown().await;
    ended.map_err(|error| anyhow::anyhow!("the MCP session stopped: {error}"))
}
