//! The read tools refuse each request that writes and name the change action that
//! makes it.

use provenance_porcelain::action_input::unknown_field_message;
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
        let takes = schema.get("properties").and_then(Value::as_object);
        let takes = takes
            .into_iter()
            .flat_map(Map::keys)
            .map(String::as_str)
            .filter(|name| *name != "notify");
        let message = unknown_field_message("get", "notify", takes);
        crate::surface::detail_error(json!({
            "kind": "invalid_input",
            "field": "notify",
            "reason": "unknown_field",
            "message": format!(
                "{message}; register a client thread with the change tool, action notify"
            ),
        }))
    })
}
