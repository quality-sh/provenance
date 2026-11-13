//! The read tools refuse each request that writes and name the change action that
//! makes it.

use rmcp::model::CallToolResult;
use serde_json::{json, Map, Value};

/// The refusal of a get call that writes: `get` with `notify` registers a client thread.
/// The tool annotations say the same; this check holds when a client does not read them.
/// The message names the fields of `schema`, the input schema of get.
#[provenance_macros::rule("rule_mcp_tools_read_or_write")]
pub fn get_refusal(
    schema: &Map<String, Value>,
    arguments: &Map<String, Value>,
) -> Option<CallToolResult> {
    arguments.contains_key("notify").then(|| {
        let takes = schema
            .get("properties")
            .and_then(Value::as_object)
            .map_or_default(|properties| {
                properties
                    .keys()
                    .filter(|name| *name != "notify")
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            });
        crate::surface::detail_error(json!({
            "kind": "invalid_input",
            "field": "notify",
            "reason": "unknown_field",
            "message": format!(
                "get does not take notify; it takes: {takes}; register a client thread with \
                 the change tool, action notify"
            ),
        }))
    })
}
