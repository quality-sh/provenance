//! MCP envelope bindings for the shared api action.

use crate::StatementHost;
use provenance_porcelain::api::{
    render_discovery_readable, ApiArguments, ApiError, ApiOutcome, ApiRequest,
};
use provenance_porcelain::Porcelain;
use rmcp::model::{CallToolResult, Content, Tool};
use serde_json::Value;

pub fn tool() -> Tool {
    let mut input = provenance_porcelain::api::input_schema();
    input.as_object_mut().map(|object| object.remove("$schema"));
    let mut output = provenance_porcelain::api::output_schema();
    output
        .as_object_mut()
        .map(|object| object.remove("$schema"));
    let mut tool = Tool::new(
        "api",
        provenance_porcelain::api::API_DESCRIPTION,
        input
            .as_object()
            .expect("api input schema is an object")
            .clone(),
    );
    tool.output_schema = Some(
        output
            .as_object()
            .expect("api output schema is an object")
            .clone()
            .into(),
    );
    tool
}

/// Parse one api tool call's structured arguments and run the shared action,
/// so MCP callers select the same paths, methods, and bodies as the CLI.
#[provenance_macros::rule("rule_porcelain_mcp_api_arguments")]
pub async fn call(
    host: &StatementHost,
    arguments: Option<serde_json::Map<String, Value>>,
) -> CallToolResult {
    let arguments = arguments.unwrap_or_default();
    let parsed: ApiArguments = match serde_json::from_value(Value::Object(arguments)) {
        Ok(value) => value,
        Err(_) => return refused(&ApiError::invalid_options()),
    };
    let request = match ApiRequest::from(parsed) {
        Ok(request) => request,
        Err(error) => return refused(&error),
    };
    let service = Porcelain::new(crate::porcelain::HostApiPort::new(host.clone()));
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

fn refused(error: &ApiError) -> CallToolResult {
    CallToolResult::structured_error(error.failure.clone())
}
