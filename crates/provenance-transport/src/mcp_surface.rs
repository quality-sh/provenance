//! Shared MCP call and tool wiring.

use crate::{StatementHost, MAX_BODY_BYTES};
use provenance_core::protocol::{failure::ErasedFailure, ResponseMeta};
use rmcp::{
    model::{CallToolResult, Tool},
    ErrorData,
};
use serde_json::{Map, Value};
use std::future::Future;

#[derive(Clone, Copy)]
pub(crate) enum SchemaDeclaration {
    /// Keep the schema draft declaration in the advertised tool schema.
    Keep,
    /// Remove the schema draft declaration for clients that reject it.
    Remove,
}

/// Admit one MCP call, enforce its input limit, and keep its permit until completion.
pub(crate) async fn admitted_call<F, Fut>(
    host: &StatementHost,
    arguments: Option<Map<String, Value>>,
    invoke: F,
) -> Result<CallToolResult, ErrorData>
where
    F: FnOnce(Map<String, Value>) -> Fut,
    Fut: Future<Output = CallToolResult>,
{
    let _admission = match host.admit() {
        Ok(permit) => permit,
        Err(failure) => return Ok(failure_result(failure)),
    };
    let arguments = arguments.unwrap_or_default();
    let encoded = serde_json::to_vec(&arguments)
        .map_err(|_| ErrorData::internal_error("Cannot encode input", None))?;
    if encoded.len() > MAX_BODY_BYTES {
        return Ok(failure_result(ErasedFailure::new(
            None,
            provenance_core::protocol::failure::OperationFailure::InvalidInput {
                field: None,
                reason: provenance_core::protocol::failure::InvalidInputReason::TooLarge,
            },
        )));
    }
    Ok(invoke(arguments).await)
}

/// Build one MCP tool from its JSON input and output schemas.
pub(crate) fn tool(
    name: &'static str,
    description: &'static str,
    mut input: Value,
    mut output: Value,
    schema_declaration: SchemaDeclaration,
) -> Tool {
    if matches!(schema_declaration, SchemaDeclaration::Remove) {
        input.as_object_mut().map(|schema| schema.remove("$schema"));
        output
            .as_object_mut()
            .map(|schema| schema.remove("$schema"));
    }
    let mut tool = Tool::new(
        name,
        description,
        input
            .as_object()
            .expect("MCP input schema is an object")
            .clone(),
    );
    tool.output_schema = Some(
        output
            .as_object()
            .expect("MCP output schema is an object")
            .clone()
            .into(),
    );
    tool
}

/// Return one MCP error result with the shared response envelope.
pub(crate) fn error_result(error: Value, meta: Value) -> CallToolResult {
    CallToolResult::structured_error(serde_json::json!({"error": error, "meta": meta}))
}

/// Preserve the error and metadata from one erased operation failure.
pub(crate) fn failure_result(failure: ErasedFailure) -> CallToolResult {
    error_result(
        failure.error,
        serde_json::to_value(failure.meta).expect("response metadata is JSON"),
    )
}

/// Return an error result with empty response metadata.
pub(crate) fn detail_error(error: Value) -> CallToolResult {
    error_result(
        error,
        serde_json::to_value(ResponseMeta::default()).expect("response metadata is JSON"),
    )
}

/// Normalize an existing response envelope through the shared constructor.
pub(crate) fn envelope_error(envelope: &Value) -> CallToolResult {
    error_result(
        envelope["error"].clone(),
        envelope
            .get("meta")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    )
}
