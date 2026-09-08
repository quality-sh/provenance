use crate::{StatementHost, MAX_BODY_BYTES};
use provenance_core::protocol::failure::{FailureEnvelope, InvalidInputReason, OperationFailure};
use provenance_store::operations::catalog;
use rmcp::{
    model::{
        CallToolRequestParam, CallToolResult, ErrorCode, ListToolsResult, PaginatedRequestParam,
        ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
    ErrorData, RoleServer, ServerHandler,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Invocation {
    protocol_version: u32,
    call: Value,
}

impl ServerHandler for StatementHost {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..ServerInfo::default()
        }
    }

    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParam>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = catalog::definitions()
            .into_iter()
            .map(|definition| {
                let mut tool = Tool::new(
                    definition.name,
                    "Check a descriptive statement.",
                    definition
                        .mcp_input_schema()
                        .as_object()
                        .expect("object input schema")
                        .clone(),
                );
                tool.output_schema = Some(
                    definition
                        .success_schema
                        .as_object()
                        .expect("object output schema")
                        .clone()
                        .into(),
                );
                tool
            })
            .collect();
        Ok(ListToolsResult {
            tools,
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        if !catalog::definitions()
            .iter()
            .any(|entry| entry.name == request.name)
        {
            return Err(ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                "Unknown tool",
                None,
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
                Ok(value) => CallToolResult::structured(value),
                Err(failure) => CallToolResult::structured_error(
                    serde_json::to_value(failure).expect("failure is JSON"),
                ),
            },
        )
    }
}
