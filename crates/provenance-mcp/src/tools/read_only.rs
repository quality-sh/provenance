//! The read tools refuse each request that writes and name the change action that
//! makes it.

use provenance_porcelain::api::ApiInput;
use provenance_transport::porcelain::api_port::route_writes;
use rmcp::model::CallToolResult;
use serde_json::{json, Map, Value};

/// A request to a read tool that this check examines.
#[derive(Clone, Copy)]
pub enum Read<'a> {
    /// The arguments of a get call.
    Get(&'a Map<String, Value>),
    /// One parsed api call.
    Api(&'a ApiInput),
}

/// The refusal of a read tool request that writes: `get` with `notify` registers a
/// client thread, and `api` with a route that writes changes the graph. The tool
/// annotations say the same; this check holds when a client does not read them.
#[provenance_macros::rule("rule_mcp_tools_read_or_write")]
pub fn refusal(read: Read<'_>) -> Option<CallToolResult> {
    let (field, reason, message) = match read {
        Read::Get(arguments) if arguments.contains_key("notify") => (
            "notify",
            "unknown_field",
            "get only reads; register a client thread with the change tool, action notify"
                .to_owned(),
        ),
        Read::Api(input) if route_writes(input.method, &input.path) == Some(true) => (
            "path",
            "invalid_value",
            format!(
                "{} {} writes; call it with the change tool, action api",
                input.method.as_str().to_uppercase(),
                input.path
            ),
        ),
        Read::Get(_) | Read::Api(_) => return None,
    };
    Some(crate::surface::detail_error(json!({
        "kind": "invalid_input",
        "field": field,
        "reason": reason,
        "message": message,
    })))
}
