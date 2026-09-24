//! MCP-owned bindings for shared Porcelain capabilities.

use provenance_porcelain::check::CheckInput;
use provenance_porcelain::get::{GetInput, ReadError};
use rmcp::model::{CallToolResult, Content};
use serde_json::Value;

pub(crate) mod authoring;
mod authoring_mcp;
mod discussion_port;
mod get_port;
mod search_port;
pub use authoring::{Action, ActionError, TargetRoute};
pub(super) use authoring_mcp::{call as call_authoring, tools as authoring_tools};
pub use discussion_port::HostDiscussionPort;
pub use get_port::HostGetPort;

pub(crate) fn discussion_is_available(host: &crate::StatementHost, action: Action) -> bool {
    discussion_port::is_available(host, action)
}

pub(crate) fn discussion_tools(host: &crate::StatementHost) -> Vec<rmcp::model::Tool> {
    Action::DISCUSSION
        .into_iter()
        .filter(|action| discussion_is_available(host, *action))
        .map(|action| {
            let mut input = provenance_porcelain::discussion::input_schema(action);
            input.as_object_mut().map(|object| object.remove("$schema"));
            let mut output = provenance_porcelain::discussion::output_schema();
            output
                .as_object_mut()
                .map(|object| object.remove("$schema"));
            let mut tool = rmcp::model::Tool::new(
                action.as_str(),
                provenance_porcelain::action::description(action),
                input
                    .as_object()
                    .expect("Discussion input schema is an object")
                    .clone(),
            );
            tool.output_schema = Some(
                output
                    .as_object()
                    .expect("Discussion output schema is an object")
                    .clone()
                    .into(),
            );
            tool
        })
        .collect()
}

pub(crate) async fn call_discussion(
    host: &crate::StatementHost,
    action: Action,
    arguments: serde_json::Map<String, Value>,
) -> CallToolResult {
    let value = Value::Object(arguments);
    let service = provenance_porcelain::Porcelain::new(HostDiscussionPort::new(host.clone()));
    let outcome = service.execute_discussion(action, value).await;
    match outcome {
        Ok(outcome) => {
            let readable = provenance_porcelain::discussion::render_readable(&outcome);
            let mut result = CallToolResult::structured(
                serde_json::to_value(outcome).expect("Discussion outcome is JSON"),
            );
            result.content = vec![Content::text(readable)];
            result
        }
        Err(error) => discussion_error(&error),
    }
}

fn discussion_error(error: &ActionError) -> CallToolResult {
    let detail = match error {
        ActionError::InvalidOptions => serde_json::json!({
            "kind":"invalid_input", "field":null, "reason":"invalid_value"
        }),
        ActionError::AccessDenied => serde_json::json!({"kind":"access_denied"}),
        ActionError::OperationDetail { detail, .. } => detail.clone(),
        ActionError::NotFound => serde_json::json!({"kind":"not_found"}),
        ActionError::AmbiguousIdentity => serde_json::json!({"kind":"ambiguous_identity"}),
        ActionError::KindSelection => serde_json::json!({"kind":"invalid_options"}),
        ActionError::Operation(message) => {
            serde_json::json!({"kind":"operation_failed","message":message})
        }
    };
    CallToolResult::structured_error(serde_json::json!({
        "error": detail, "meta": {}, "message": error.to_string()
    }))
}
pub use search_port::HostSearchPort;

pub(crate) fn get_is_available(host: &crate::StatementHost) -> bool {
    get_port::is_available(host)
}

pub(crate) fn search_is_available(host: &crate::StatementHost) -> bool {
    search_port::is_available(host)
}

pub(crate) fn search_tool() -> rmcp::model::Tool {
    use provenance_store::operations::catalog;
    let mut input = catalog::operation_request_schema::<catalog::Search>();
    input.as_object_mut().map(|object| object.remove("$schema"));
    if let Some(properties) = input.get_mut("properties").and_then(Value::as_object_mut) {
        properties.remove("protocol_version");
    }
    if let Some(required) = input.get_mut("required").and_then(Value::as_array_mut) {
        required.retain(|field| field != "protocol_version");
    }
    let mut output = catalog::operation_success_schema::<catalog::Search>();
    output
        .as_object_mut()
        .map(|object| object.remove("$schema"));
    let mut tool = rmcp::model::Tool::new(
        "search",
        "Find records across the permitted kinds in the bound scope.",
        input
            .as_object()
            .expect("search schema is an object")
            .clone(),
    );
    tool.output_schema = Some(
        output
            .as_object()
            .expect("search output schema is an object")
            .clone()
            .into(),
    );
    tool
}

pub(crate) fn get_tool() -> rmcp::model::Tool {
    let schema = provenance_porcelain::get::input_schema();
    let mut tool = rmcp::model::Tool::new(
        "get",
        "Read one repository record by its repository-local ID.",
        schema.as_object().expect("get schema is an object").clone(),
    );
    tool.output_schema = Some(
        provenance_porcelain::get::output_schema()
            .as_object()
            .expect("get output schema is an object")
            .clone()
            .into(),
    );
    tool
}

pub(crate) fn check_tool() -> rmcp::model::Tool {
    let schema = provenance_porcelain::check::input_schema();
    let mut tool = rmcp::model::Tool::new(
        "check",
        "Check graph validity, statement quality, and binding coverage.",
        schema
            .as_object()
            .expect("check schema is an object")
            .clone(),
    );
    tool.output_schema = Some(
        provenance_porcelain::check::output_schema()
            .as_object()
            .expect("check output schema is an object")
            .clone()
            .into(),
    );
    tool
}

pub(crate) async fn call_check(
    host: &crate::StatementHost,
    port: std::sync::Arc<dyn provenance_porcelain::check::CheckPort>,
    arguments: serde_json::Map<String, Value>,
) -> CallToolResult {
    let Ok(arguments) = serde_json::from_value::<CheckInput>(Value::Object(arguments)) else {
        return get_error("invalid_options", "unsupported check options");
    };
    let service = provenance_porcelain::Porcelain::new(port);
    let mut input = arguments;
    if let Some((_, scope)) = host.bound_identity() {
        input = input.in_scope(scope);
    }
    let outcome = service.check(input).await;
    let summary = provenance_porcelain::check::render_readable(&outcome);
    let mut result =
        CallToolResult::structured(serde_json::to_value(outcome).expect("check outcome is JSON"));
    result.content = vec![Content::text(summary)];
    result
}

/// Returns readable and structured MCP content for one get request.
pub(crate) async fn call_get(
    host: &crate::StatementHost,
    arguments: serde_json::Map<String, Value>,
) -> CallToolResult {
    let Ok(arguments) = serde_json::from_value::<GetInput>(Value::Object(arguments)) else {
        return get_error("invalid_options", "unsupported read options");
    };
    let service = provenance_porcelain::Porcelain::new(HostGetPort::new(host.clone()));
    match service.get(arguments).await {
        Ok(outcome) => {
            let summary = provenance_porcelain::get::render_readable(&outcome)
                .expect("get outcome is readable JSON");
            let mut result = CallToolResult::structured(
                serde_json::to_value(outcome).expect("get outcome is JSON"),
            );
            result.content = vec![Content::text(summary)];
            result
        }
        Err(error) => get_error(
            match error {
                ReadError::InvalidOptions => "invalid_options",
                ReadError::NotFound => "not_found",
                ReadError::AmbiguousIdentity => "ambiguous_identity",
                ReadError::Operation(_) => "operation_failed",
            },
            &error.to_string(),
        ),
    }
}

/// Returns compact readable content and the canonical structured search result.
pub(crate) async fn call_search(
    host: &crate::StatementHost,
    arguments: serde_json::Map<String, Value>,
) -> CallToolResult {
    let Ok(mut request) =
        serde_json::from_value::<provenance_core::protocol::SearchQuery>(Value::Object(arguments))
    else {
        return get_error("invalid_options", "unsupported search options");
    };
    request.protocol_version = Some(provenance_core::SDK_PROTOCOL_VERSION);
    let service = provenance_porcelain::Porcelain::new(HostSearchPort::new(host.clone()));
    match service.search(request).await {
        Ok(response) => {
            let summary = provenance_porcelain::search::render_readable(&response);
            let mut result = CallToolResult::structured(
                serde_json::to_value(response).expect("search result is JSON"),
            );
            result.content = vec![Content::text(summary)];
            result
        }
        Err(error) => search_error(&error),
    }
}

fn search_error(error: &provenance_porcelain::search::SearchError) -> CallToolResult {
    use provenance_porcelain::search::SearchError;
    let detail = match error {
        SearchError::InvalidOptions => serde_json::json!({
            "kind":"invalid_input", "field":null, "reason":"invalid_value"
        }),
        SearchError::AccessDenied => serde_json::json!({"kind":"access_denied"}),
        SearchError::Operation { detail, .. } => detail.clone(),
    };
    CallToolResult::structured_error(serde_json::json!({
        "error": detail, "meta": {}, "message": error.to_string()
    }))
}

fn get_error(kind: &str, message: &str) -> CallToolResult {
    CallToolResult::structured_error(serde_json::json!({
        "error": {"kind": kind, "message": message}
    }))
}
