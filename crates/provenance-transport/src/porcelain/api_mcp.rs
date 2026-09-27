//! MCP envelope bindings for the shared api action.

use crate::StatementHost;
use provenance_porcelain::api::{
    render_discovery_readable, ApiArguments, ApiError, ApiOutcome, ApiRequest,
};
use rmcp::model::{CallToolResult, Content, Tool};
use serde_json::Value;

pub fn tool() -> Tool {
    crate::mcp_surface::tool(
        "api",
        provenance_porcelain::api::API_DESCRIPTION,
        provenance_porcelain::api::input_schema(),
        provenance_porcelain::api::output_schema(),
        crate::mcp_surface::SchemaDeclaration::Remove,
    )
}

/// Parse one api tool call's structured arguments and run the shared action,
/// so MCP callers select the same paths, methods, and bodies as the CLI.
#[provenance_macros::rule("rule_porcelain_mcp_api_arguments")]
pub async fn call(
    host: &StatementHost,
    arguments: Option<serde_json::Map<String, Value>>,
) -> CallToolResult {
    let mut arguments = arguments.unwrap_or_default();
    if let Some(Value::String(method)) = arguments.get_mut("method") {
        method.make_ascii_lowercase();
    }
    let Ok(parsed) = serde_json::from_value::<ApiArguments>(Value::Object(arguments.clone()))
    else {
        let field = rejected_field(&arguments);
        return refused(&ApiError::invalid_options(field.as_deref()));
    };
    let request = match ApiRequest::from(parsed) {
        Ok(request) => request,
        Err(error) => return refused(&error),
    };
    let service = host.porcelain().api();
    match service.execute_api(request).await {
        Ok(ApiOutcome::Catalog(catalog)) => {
            let mut result = CallToolResult::structured(
                serde_json::to_value(&catalog).expect("catalog is JSON"),
            );
            result.content = vec![Content::text(render_discovery_readable(&catalog))];
            result
        }
        Ok(ApiOutcome::Invoked(value)) => CallToolResult::structured(value),
        Err(error) => refused(&error),
    }
}

/// Name the first argument that the typed contract refuses on its own.
fn rejected_field(arguments: &serde_json::Map<String, Value>) -> Option<String> {
    arguments
        .iter()
        .find(|(name, value)| {
            let mut single = serde_json::Map::new();
            single.insert((*name).clone(), (*value).clone());
            serde_json::from_value::<ApiArguments>(Value::Object(single)).is_err()
        })
        .map(|(name, _)| name.clone())
}

fn refused(error: &ApiError) -> CallToolResult {
    crate::mcp_surface::envelope_error(&error.failure)
}
