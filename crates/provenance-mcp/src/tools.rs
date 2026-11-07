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
        crate::exposed::name(Command::Search),
        Command::Search.declaration().description,
        Effect::Reads,
        crate::derived::input_schema(Command::Search),
        provenance_porcelain::search::output_schema(),
        SchemaDeclaration::Remove,
    )
}

pub fn get_tool() -> rmcp::model::Tool {
    crate::surface::tool(
        crate::exposed::name(Command::Get),
        Command::Get.declaration().description,
        Effect::Reads,
        crate::derived::input_schema(Command::Get),
        provenance_porcelain::get::output_schema(),
        SchemaDeclaration::Keep,
    )
}

pub fn trace_tool() -> rmcp::model::Tool {
    crate::surface::tool(
        crate::exposed::name(Command::Trace),
        Command::Trace.declaration().description,
        Effect::Reads,
        crate::derived::input_schema(Command::Trace),
        provenance_porcelain::get::output_schema(),
        SchemaDeclaration::Keep,
    )
}

pub fn check_tool() -> rmcp::model::Tool {
    crate::surface::tool(
        crate::exposed::name(Command::Check),
        Command::Check.declaration().description,
        Effect::Reads,
        crate::derived::input_schema(Command::Check),
        provenance_porcelain::check::output_schema(),
        SchemaDeclaration::Keep,
    )
}

pub async fn call_check(host: &StatementHost, arguments: Map<String, Value>) -> CallToolResult {
    let Ok(mut input) = serde_json::from_value::<CheckInput>(Value::Object(arguments)) else {
        return get_error("invalid_options", "unsupported check options");
    };
    if let Err(reason) = input.validate() {
        return get_error(
            "invalid_options",
            &format!("unsupported check options: {reason}"),
        );
    }
    // The caller can read only the scope that the server binds.
    if let Some((_, scope)) = host.bound_identity() {
        if input.scope.as_ref().is_some_and(|asked| *asked != scope) {
            return get_error("access_denied", "this server checks only its bound scope");
        }
        input = input.in_scope(scope);
    }
    let outcome = host.porcelain().check().check(input).await;
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
    crate::surface::detail_error(with_message(detail, recovery_message(error)))
}

/// The refusal message followed by the recovery inputs as tool arguments.
fn recovery_message(error: &provenance_porcelain::search::SearchError) -> String {
    let inputs = error.recovery_inputs();
    if inputs.is_empty() {
        return error.to_string();
    }
    let forms = inputs.iter().map(Value::to_string).collect::<Vec<_>>();
    format!("{error}; call search with {}", forms.join(" or "))
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
