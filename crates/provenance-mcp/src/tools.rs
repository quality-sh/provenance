//! MCP tools for the shared Porcelain capabilities. `change` and the `api` plumbing tool
//! write; each other tool only reads.

use crate::surface::{Effect, SchemaDeclaration};
use provenance_porcelain::check::CheckInput;
use provenance_porcelain::get::{GetInput, GetOutcome, ReadError, TraceInput};
use provenance_porcelain::registry::Command;
use provenance_transport::StatementHost;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{Map, Value};

mod api;
mod authoring;
mod change;
mod discussions;
mod notify;
mod read_only;
pub use api::{call as call_api, lowercase_method, tool as api_tool};
pub use change::{
    input as change_input, refusal as change_refusal, run as call_change, tool as change_tool,
};
pub use discussions::{
    refusal as discussions_refusal, run as call_discussion, tool as discussions_tool,
};
pub use read_only::get_refusal;

pub fn search_tool() -> rmcp::model::Tool {
    crate::surface::tool(
        Command::Search.declaration().name,
        Command::Search.declaration().description,
        Effect::Reads,
        provenance_porcelain::search::input_schema(),
        provenance_porcelain::search::output_schema(),
        SchemaDeclaration::Remove,
    )
}

pub fn get_tool() -> rmcp::model::Tool {
    crate::surface::tool(
        Command::Get.declaration().name,
        Command::Get.declaration().description,
        Effect::Reads,
        provenance_porcelain::get::input_schema(),
        provenance_porcelain::get::output_schema(),
        SchemaDeclaration::Keep,
    )
}

pub fn trace_tool() -> rmcp::model::Tool {
    crate::surface::tool(
        Command::Trace.declaration().name,
        Command::Trace.declaration().description,
        Effect::Reads,
        provenance_porcelain::get::trace_input_schema(),
        provenance_porcelain::get::output_schema(),
        SchemaDeclaration::Keep,
    )
}

pub fn check_tool() -> rmcp::model::Tool {
    crate::surface::tool(
        Command::Check.declaration().name,
        Command::Check.declaration().description,
        Effect::Reads,
        provenance_porcelain::check::input_schema(),
        provenance_porcelain::check::output_schema(),
        SchemaDeclaration::Keep,
    )
}

pub async fn call_check(host: &StatementHost, arguments: Map<String, Value>) -> CallToolResult {
    let Ok(arguments) = serde_json::from_value::<CheckInput>(Value::Object(arguments)) else {
        return get_error("invalid_options", "unsupported check options");
    };
    let service = host.porcelain().check();
    let mut input = arguments;
    if let Some((_, scope)) = host.bound_identity() {
        input = input.in_scope(scope);
    }
    let outcome = service.check(input).await;
    let summary = provenance_porcelain::check::render_readable(&outcome);
    let mut result =
        CallToolResult::structured(serde_json::to_value(outcome).expect("check outcome is JSON"));
    result.content = vec![ContentBlock::text(summary)];
    result
}

/// Returns readable and structured MCP content for one get request.
pub async fn call_get(host: &StatementHost, arguments: Map<String, Value>) -> CallToolResult {
    let Ok(arguments) = serde_json::from_value::<GetInput>(Value::Object(arguments)) else {
        return crate::surface::envelope_error(&ReadError::InvalidOptions.failure());
    };
    get_result(host.porcelain().get().get(arguments).await)
}

/// Returns the same content as the get trace view for one trace request.
pub async fn call_trace(host: &StatementHost, arguments: Map<String, Value>) -> CallToolResult {
    let Ok(arguments) = serde_json::from_value::<TraceInput>(Value::Object(arguments)) else {
        return crate::surface::envelope_error(&ReadError::InvalidOptions.failure());
    };
    get_result(host.porcelain().get().trace(arguments).await)
}

fn get_result(outcome: Result<GetOutcome, ReadError>) -> CallToolResult {
    match outcome {
        Ok(outcome) => {
            let summary = provenance_porcelain::get::render_readable(&outcome)
                .expect("get outcome is readable JSON");
            let mut result = CallToolResult::structured(
                serde_json::to_value(outcome).expect("get outcome is JSON"),
            );
            result.content = vec![ContentBlock::text(summary)];
            result
        }
        Err(error) => crate::surface::envelope_error(&error.failure()),
    }
}

/// Returns compact readable content and the canonical structured search result.
#[provenance_macros::rule("rule_search_file_same_on_cli_and_mcp")]
pub async fn call_search(host: &StatementHost, arguments: Map<String, Value>) -> CallToolResult {
    let Ok(input) = serde_json::from_value::<provenance_porcelain::search::SearchInput>(
        Value::Object(arguments),
    ) else {
        return get_error("invalid_options", "unsupported search options");
    };
    let service = host.porcelain().search();
    match service.search(input).await {
        Ok(outcome) => {
            let summary = provenance_porcelain::search::render_readable(&outcome);
            let mut result = CallToolResult::structured(
                serde_json::to_value(outcome).expect("search result is JSON"),
            );
            result.content = vec![ContentBlock::text(summary)];
            result
        }
        Err(error) => search_error(&error),
    }
}

fn search_error(error: &provenance_porcelain::search::SearchError) -> CallToolResult {
    use provenance_porcelain::search::SearchError;
    let detail = match error {
        SearchError::InvalidOptions { .. } => serde_json::json!({
            "kind":"invalid_input", "field":null, "reason":"invalid_value"
        }),
        SearchError::InvalidCodeLookup { field, .. } => serde_json::json!({
            "kind":"invalid_input", "field":format!("request.{field}"), "reason":"invalid_value"
        }),
        SearchError::AccessDenied => serde_json::json!({"kind":"access_denied"}),
        SearchError::Operation { detail, .. } => detail.clone(),
    };
    crate::surface::detail_error(with_message(detail, error.to_string()))
}

fn get_error(kind: &str, message: &str) -> CallToolResult {
    crate::surface::detail_error(serde_json::json!({"kind": kind, "message": message}))
}

fn with_message(mut detail: Value, message: String) -> Value {
    detail
        .as_object_mut()
        .expect("error detail is an object")
        .entry("message")
        .or_insert(Value::String(message));
    detail
}
