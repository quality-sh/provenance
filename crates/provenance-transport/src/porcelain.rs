//! MCP-owned bindings for shared Porcelain capabilities.

use provenance_porcelain::{Action, Outcome, RecordRequest};
use rmcp::model::{CallToolResult, Content};
use serde::{Deserialize, Serialize};

const ACTION_NAMES: &[&str] = &["get", "check"];

/// Return the Porcelain action names exposed by the MCP binding.
pub const fn action_names() -> &'static [&'static str] {
    ACTION_NAMES
}

/// MCP input for the `get` Porcelain action.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GetArguments {
    /// The repository-local record ID.
    pub target: String,
}

impl GetArguments {
    /// Translate the MCP input into a shared semantic request.
    pub fn into_request(self) -> RecordRequest {
        RecordRequest::new(self.target, Action::Get)
    }
}

/// Render a shared outcome in the MCP-owned result shape.
pub fn render<T: Serialize>(outcome: Outcome<T>) -> serde_json::Result<CallToolResult> {
    let mut result = CallToolResult::structured(serde_json::to_value(outcome.data)?);
    result.content = vec![Content::text(outcome.summary)];
    Ok(result)
}
