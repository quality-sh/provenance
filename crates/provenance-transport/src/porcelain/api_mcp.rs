//! MCP envelope bindings for the shared api action.

use crate::StatementHost;
use rmcp::model::{CallToolResult, Tool};
use serde_json::Value;

pub(super) fn tool() -> Tool {
    todo!("api tool schema")
}

pub(super) async fn call(
    host: &StatementHost,
    arguments: Option<serde_json::Map<String, Value>>,
) -> CallToolResult {
    let _ = (host, arguments);
    todo!("api tool call")
}
