//! MCP envelope bindings for the shared api action. The api tool reads and writes, as the
//! CLI api command does.

use crate::surface::{Effect, SchemaDeclaration};
use provenance_porcelain::api::{
    render_discovery_readable, ApiArguments, ApiError, ApiOutcome, ApiRequest, SeeMore,
};
use provenance_porcelain::registry::Command;
use provenance_transport::StatementHost;
use rmcp::model::{CallToolResult, ContentBlock, Tool};
use serde_json::{Map, Value};

pub fn tool() -> Tool {
    crate::surface::tool(
        crate::exposed::name(Command::Api),
        Command::Api.declaration().description,
        Effect::Plumbing,
        crate::derived::input_schema(provenance_porcelain::registry::Command::Api),
        provenance_porcelain::api::output_schema(),
        SchemaDeclaration::Remove,
    )
}

/// The api fields that select more catalog routes.
const SEE_MORE: SeeMore = SeeMore {
    filter: "{\"filter\":\"<text>\"}",
    limit: "{\"limit\":<number>}",
    route: "{\"filter\":\"<method> <path>\"}",
};

/// The method of an api call in lower case, as the schema gives it.
pub fn lowercase_method(arguments: &mut Map<String, Value>) {
    if let Some(Value::String(method)) = arguments.get_mut("method") {
        method.make_ascii_lowercase();
    }
}

/// Parse one api tool call's structured arguments and run the shared action,
/// so MCP callers select the same paths, methods, and bodies as the CLI.
///
/// The catalog answer is bounded as the CLI text answer is, because the client reads
/// it whole, and it has schemas only when it lists one route.
#[provenance_macros::rule("rule_porcelain_mcp_api_arguments")]
pub async fn call(host: &StatementHost, arguments: Map<String, Value>) -> CallToolResult {
    let Ok(parsed) = serde_json::from_value::<ApiArguments>(Value::Object(arguments.clone()))
    else {
        return refused(&ApiError::invalid_options(
            rejected_field(&arguments).as_deref(),
        ));
    };
    let request = match ApiRequest::from(parsed) {
        Ok(request) => request,
        Err(error) => return refused(&error),
    };
    match host.porcelain().api().execute_api(request).await {
        Ok(ApiOutcome::Catalog(selection)) => {
            let answer = selection.bounded();
            let text = render_discovery_readable(&answer, SEE_MORE);
            let mut result =
                CallToolResult::structured(serde_json::to_value(&answer).expect("catalog is JSON"));
            result.content = vec![ContentBlock::text(text)];
            result
        }
        Ok(ApiOutcome::Invoked(value)) => CallToolResult::structured(value),
        Err(error) => refused(&error),
    }
}

/// Name the first argument that the typed contract refuses on its own.
fn rejected_field(arguments: &Map<String, Value>) -> Option<String> {
    arguments
        .iter()
        .find(|(name, value)| {
            let mut single = Map::new();
            single.insert((*name).clone(), (*value).clone());
            serde_json::from_value::<ApiArguments>(Value::Object(single)).is_err()
        })
        .map(|(name, _)| name.clone())
}

fn refused(error: &ApiError) -> CallToolResult {
    crate::surface::envelope_error(&error.failure)
}
