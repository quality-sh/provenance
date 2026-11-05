//! MCP tools for the shared Porcelain capabilities. Each tool only reads or only writes:
//! `change` is the one tool that writes.

use crate::surface::{Effect, SchemaDeclaration};
use provenance_porcelain::check::CheckInput;
use provenance_porcelain::get::{GetInput, GetOutcome, ReadError, TraceInput};
use provenance_porcelain::registry::Command;
use provenance_transport::{porcelain::Action, StatementHost};
use rmcp::model::{CallToolResult, ContentBlock, Tool};
use serde_json::{Map, Value};

mod api;
mod authoring;
mod change;
mod discussions;
mod notify;
mod read_only;
pub use api::tool as api_tool;
pub use change::tool as change_tool;
pub use discussions::tool as discussions_tool;

/// Runs one call of a listed tool. A read tool refuses a request that writes. Each
/// tool checks the arguments against its listed input schema; a tool with an
/// action argument first checks that the fields agree with the action.
pub async fn call(
    host: &StatementHost,
    command: Command,
    listed: &Tool,
    mut arguments: Map<String, Value>,
) -> CallToolResult {
    let schema = Value::Object((*listed.input_schema).clone());
    if command == Command::Api {
        api::lowercase_method(&mut arguments);
    }
    if command == Command::Get {
        if let Some(refused) = read_only::refusal(read_only::Read::Get(&arguments)) {
            return refused;
        }
    }
    match command {
        Command::Change => return change::call(host, &schema, arguments).await,
        Command::Action(Action::Discussions) => {
            return discussions::call(host, &schema, arguments).await
        }
        _ => {}
    }
    if let Some(refused) = crate::arguments::refusal(&listed.name, &schema, &arguments) {
        return crate::surface::detail_error(refused);
    }
    match command {
        Command::Get => call_get(host, arguments).await,
        Command::Trace => call_trace(host, arguments).await,
        Command::Search => call_search(host, arguments).await,
        Command::Api => api::call(host, arguments).await,
        Command::Check => {
            let port = host.check_port().expect("a listed check tool has a port");
            call_check(host, port.clone(), arguments).await
        }
        _ => unreachable!("the server lists no other tool"),
    }
}

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

async fn call_check(
    host: &StatementHost,
    port: std::sync::Arc<dyn provenance_porcelain::check::CheckPort>,
    arguments: Map<String, Value>,
) -> CallToolResult {
    let Ok(arguments) = serde_json::from_value::<CheckInput>(Value::Object(arguments)) else {
        return get_error("invalid_options", "unsupported check options");
    };
    let service = host.porcelain().check(port);
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
async fn call_get(host: &StatementHost, arguments: Map<String, Value>) -> CallToolResult {
    let Ok(arguments) = serde_json::from_value::<GetInput>(Value::Object(arguments)) else {
        return crate::surface::envelope_error(&ReadError::InvalidOptions.failure());
    };
    get_result(host.porcelain().get().get(arguments).await)
}

/// Returns the same content as the get trace view for one trace request.
async fn call_trace(host: &StatementHost, arguments: Map<String, Value>) -> CallToolResult {
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
async fn call_search(host: &StatementHost, arguments: Map<String, Value>) -> CallToolResult {
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
