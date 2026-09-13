use crate::{StatementHost, MAX_BODY_BYTES};
use provenance_core::protocol::failure::{
    ErasedFailure as FailureEnvelope, InvalidInputReason, OperationFailure,
};
use provenance_store::operations::catalog;
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResult, ErrorCode, ListToolsResult, PaginatedRequestParams,
        ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
    ErrorData, RoleServer, ServerHandler,
};
use serde::Deserialize;
use serde_json::Value;
use std::future::Future;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Invocation {
    protocol_version: u32,
    call: Value,
}

impl ServerHandler for StatementHost {
    fn get_info(&self) -> ServerInfo {
        // The type is non-exhaustive in rmcp 1.4, so build from the default.
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }

    /// Listing tools is synchronous catalog work, so this returns a ready
    /// future instead of declaring an `async fn` that never awaits.
    fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + '_ {
        let tools = catalog::definitions()
            .into_iter()
            .filter(|definition| self.advertises(definition.name))
            .map(|definition| {
                let mut tool = Tool::new(
                    definition.name,
                    "Invoke the shared operation.",
                    definition
                        .mcp_input_schema()
                        .as_object()
                        .expect("object input schema")
                        .clone(),
                );
                tool.output_schema = Some(
                    definition
                        .mcp_output_schema()
                        .as_object()
                        .expect("object output schema")
                        .clone()
                        .into(),
                );
                tool
            })
            .collect();
        std::future::ready(Ok(ListToolsResult {
            tools,
            ..Default::default()
        }))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        if !catalog::contains(&request.name) {
            return Err(ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                "Unknown tool",
                None,
            ));
        }
        if !self.advertises(&request.name) {
            return Ok(CallToolResult::structured_error(
                serde_json::to_value(FailureEnvelope::new(
                    Some(&request.name),
                    OperationFailure::AccessDenied,
                ))
                .expect("failure is JSON"),
            ));
        }
        let _admission = match self.admit() {
            Ok(permit) => permit,
            Err(failure) => {
                return Ok(CallToolResult::structured_error(
                    serde_json::to_value(failure).expect("failure is JSON"),
                ))
            }
        };
        let arguments = Value::Object(request.arguments.unwrap_or_default());
        if serde_json::to_vec(&arguments)
            .map_err(|_| ErrorData::internal_error("Cannot encode input", None))?
            .len()
            > MAX_BODY_BYTES
        {
            return Ok(CallToolResult::structured_error(
                serde_json::to_value(FailureEnvelope::new(
                    Some(&request.name),
                    OperationFailure::InvalidInput {
                        field: None,
                        reason: InvalidInputReason::TooLarge,
                    },
                ))
                .expect("failure is JSON"),
            ));
        }
        let invocation: Invocation = serde_json::from_value(arguments)
            .map_err(|_| ErrorData::invalid_params("Expected protocol_version and call", None))?;
        Ok(
            match self
                .invoke(
                    request.name.into_owned(),
                    invocation.protocol_version,
                    invocation.call,
                )
                .await
            {
                Ok(value) => CallToolResult::structured(if value.is_array() {
                    serde_json::json!({"result":value})
                } else {
                    value
                }),
                Err(failure) => CallToolResult::structured_error(
                    serde_json::to_value(failure).expect("failure is JSON"),
                ),
            },
        )
    }
}
