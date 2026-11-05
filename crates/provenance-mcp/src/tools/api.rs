//! MCP envelope bindings for the shared api action. The api tool only reads; the api
//! action of the change tool makes the calls that write.

use crate::surface::{Effect, SchemaDeclaration};
use provenance_porcelain::api::{
    render_discovery_readable, ApiArguments, ApiError, ApiInput, ApiOutcome, ApiRequest,
};
use provenance_porcelain::registry::Command;
use provenance_transport::porcelain::api_port::route_writes;
use provenance_transport::StatementHost;
use rmcp::model::{CallToolResult, ContentBlock, Tool};
use serde_json::{json, Map, Value};

pub fn tool() -> Tool {
    crate::surface::tool(
        Command::Api.declaration().name,
        Command::Api.declaration().description,
        Effect::Reads,
        provenance_porcelain::api::input_schema(),
        provenance_porcelain::api::output_schema(),
        SchemaDeclaration::Remove,
    )
}

/// The fields of the api action of the change tool: one api call in `request`.
pub fn write_input_schema() -> Value {
    let mut request = provenance_porcelain::api::input_schema();
    let definitions = request
        .as_object_mut()
        .and_then(|object| object.remove("$defs"));
    for keyword in ["$schema", "title"] {
        request.as_object_mut().map(|object| object.remove(keyword));
    }
    request["description"] = json!("One api call that writes, with the fields of the api tool.");
    let mut schema = json!({"properties": {"request": request}, "required": ["request"]});
    if let Some(definitions) = definitions {
        schema["$defs"] = definitions;
    }
    schema
}

/// The method of an api call in lower case, as the schema gives it: `method` in the api
/// tool, `request.method` in the change tool.
pub fn lowercase_method(arguments: &mut Map<String, Value>) {
    let method = if arguments.get("request").is_some_and(Value::is_object) {
        arguments
            .get_mut("request")
            .and_then(|request| request.get_mut("method"))
    } else {
        arguments.get_mut("method")
    };
    if let Some(Value::String(method)) = method {
        method.make_ascii_lowercase();
    }
}

/// Parse one api tool call's structured arguments and run the shared action,
/// so MCP callers select the same paths, methods, and bodies as the CLI.
#[provenance_macros::rule("rule_porcelain_mcp_api_arguments")]
pub async fn call(host: &StatementHost, arguments: Map<String, Value>) -> CallToolResult {
    let request = match request(&arguments) {
        Ok(request) => request,
        Err(error) => return refused(&error),
    };
    if let ApiRequest::Invoke(input) = &request {
        if let Some(refused) = super::read_only::refusal(super::read_only::Read::Api(input)) {
            return refused;
        }
    }
    execute(host, request).await
}

/// Runs one api call that writes: the `request` field of the change tool.
pub async fn call_write(host: &StatementHost, mut arguments: Map<String, Value>) -> CallToolResult {
    let Some(Value::Object(fields)) = arguments.remove("request") else {
        return refused(&ApiError::invalid_options(Some("request")));
    };
    let request = match request(&fields) {
        Ok(ApiRequest::Invoke(input)) => input,
        Ok(ApiRequest::Discover) => {
            return crate::surface::detail_error(json!({
                "kind": "invalid_input",
                "field": "request.path",
                "reason": "required",
                "message": "the api action needs request.path; the api tool lists the routes",
            }))
        }
        Err(error) => return refused(&in_request(error)),
    };
    if route_writes(request.method, &request.path) == Some(false) {
        return wrong_tool(
            &request,
            "request.path",
            "only reads; call it with the api tool",
        );
    }
    execute(host, ApiRequest::Invoke(request)).await
}

fn request(arguments: &Map<String, Value>) -> Result<ApiRequest, ApiError> {
    let Ok(parsed) = serde_json::from_value::<ApiArguments>(Value::Object(arguments.clone()))
    else {
        return Err(ApiError::invalid_options(
            rejected_field(arguments).as_deref(),
        ));
    };
    ApiRequest::from(parsed)
}

async fn execute(host: &StatementHost, request: ApiRequest) -> CallToolResult {
    match host.porcelain().api().execute_api(request).await {
        Ok(ApiOutcome::Catalog(catalog)) => {
            let mut result = CallToolResult::structured(
                serde_json::to_value(&catalog).expect("catalog is JSON"),
            );
            result.content = vec![ContentBlock::text(render_discovery_readable(&catalog))];
            result
        }
        Ok(ApiOutcome::Invoked(value)) => CallToolResult::structured(value),
        Err(error) => refused(&error),
    }
}

/// The refusal of a call that the other tool makes.
fn wrong_tool(input: &ApiInput, field: &str, reason: &str) -> CallToolResult {
    crate::surface::detail_error(json!({
        "kind": "invalid_input",
        "field": field,
        "reason": "invalid_value",
        "message": format!("{} {} {reason}", input.method.as_str().to_uppercase(), input.path),
    }))
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

/// The refusal with its field named inside the `request` field.
fn in_request(mut error: ApiError) -> ApiError {
    if let Some(Value::String(field)) = error.failure.pointer_mut("/error/field") {
        *field = format!("request.{field}");
    }
    error
}

fn refused(error: &ApiError) -> CallToolResult {
    crate::surface::envelope_error(&error.failure)
}
