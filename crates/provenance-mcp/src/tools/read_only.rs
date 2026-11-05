//! The read tools refuse each request that writes and name the change action that
//! makes it.

use rmcp::model::CallToolResult;
use serde_json::{json, Map, Value};

/// The refusal of a get call that writes: `get` with `notify` registers a client thread.
/// The tool annotations say the same; this check holds when a client does not read them.
#[provenance_macros::rule("rule_mcp_tools_read_or_write")]
pub fn get_refusal(arguments: &Map<String, Value>) -> Option<CallToolResult> {
    arguments.contains_key("notify").then(|| {
        crate::surface::detail_error(json!({
            "kind": "invalid_input",
            "field": "notify",
            "reason": "unknown_field",
            "message": "get only reads; register a client thread with the change tool, action notify",
        }))
    })
}
