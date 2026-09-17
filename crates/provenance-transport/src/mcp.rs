use crate::{routing, StatementHost, MAX_BODY_BYTES};
use provenance_core::protocol::failure::{ErasedFailure, InvalidInputReason, OperationFailure};
use provenance_store::operations::catalog;
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResult, ErrorCode, ListToolsResult, PaginatedRequestParams,
        ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
    ErrorData, RoleServer, ServerHandler,
};
use serde_json::Value;
use std::{collections::BTreeMap, future::Future};

impl ServerHandler for StatementHost {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }

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
                    definition.description,
                    definition.mcp_input_schema().as_object().unwrap().clone(),
                );
                tool.output_schema = Some(
                    definition
                        .mcp_output_schema()
                        .as_object()
                        .unwrap()
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
        let Some(definition) = catalog::definitions()
            .into_iter()
            .find(|d| d.name == request.name)
        else {
            return Err(ErrorData::new(
                ErrorCode::METHOD_NOT_FOUND,
                "Unknown tool",
                None,
            ));
        };
        if !self.advertises(definition.name) {
            return Ok(error(ErasedFailure::new(
                None,
                OperationFailure::AccessDenied,
            )));
        }
        let _admission = match self.admit() {
            Ok(permit) => permit,
            Err(failure) => return Ok(error(failure)),
        };
        let arguments = Value::Object(request.arguments.unwrap_or_default());
        if serde_json::to_vec(&arguments)
            .map_err(|_| ErrorData::internal_error("Cannot encode input", None))?
            .len()
            > MAX_BODY_BYTES
        {
            return Ok(error(ErasedFailure::new(
                None,
                OperationFailure::InvalidInput {
                    field: None,
                    reason: InvalidInputReason::TooLarge,
                },
            )));
        }
        let (matched, data, query, headers) = match mcp_call(definition, &arguments) {
            Ok(call) => call,
            Err(failure) => return Ok(error(failure)),
        };
        Ok(
            match routing::invoke(self, &matched, data, query, &headers).await {
                Ok((value, _)) => CallToolResult::structured(value),
                Err(failure) => error(failure),
            },
        )
    }
}

type McpCall = (
    routing::Matched,
    Value,
    BTreeMap<String, String>,
    axum::http::HeaderMap,
);

fn mcp_call(
    definition: &'static catalog::Definition,
    value: &Value,
) -> Result<McpCall, ErasedFailure> {
    let mut arguments = value.as_object().cloned().ok_or_else(invalid)?;
    let data = arguments
        .remove("data")
        .unwrap_or_else(|| serde_json::json!({}));
    let mut path = BTreeMap::new();
    let mut query = BTreeMap::new();
    let mut headers = axum::http::HeaderMap::new();
    for parameter in definition.parameters() {
        let mcp_name = if parameter.location == "header" {
            parameter.name.to_ascii_lowercase().replace('-', "_")
        } else {
            parameter.name.to_owned()
        };
        let value = arguments.remove(&mcp_name);
        if parameter.required && value.is_none() {
            return Err(invalid());
        }
        let Some(value) = value else { continue };
        let text = match value {
            Value::String(value) => value,
            other => other.to_string(),
        };
        match parameter.location {
            "path" => {
                path.insert(parameter.name.to_owned(), text);
            }
            "query" => {
                query.insert(parameter.name.to_owned(), text);
            }
            "header" => {
                let name = axum::http::HeaderName::from_bytes(parameter.name.as_bytes())
                    .map_err(|_| invalid())?;
                let value = text.parse().map_err(|_| invalid())?;
                headers.insert(name, value);
            }
            _ => return Err(invalid()),
        }
    }
    if !arguments.is_empty() {
        return Err(invalid());
    }
    Ok((routing::Matched { definition, path }, data, query, headers))
}

fn invalid() -> ErasedFailure {
    ErasedFailure::new(
        None,
        OperationFailure::InvalidInput {
            field: None,
            reason: InvalidInputReason::InvalidValue,
        },
    )
}

fn error(failure: ErasedFailure) -> CallToolResult {
    CallToolResult::structured_error(serde_json::to_value(failure).expect("failure is JSON"))
}
