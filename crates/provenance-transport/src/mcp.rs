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

    /// Lists the live MCP action names.
    fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + '_ {
        let mut tools = catalog::definitions()
            .iter()
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
            .collect::<Vec<_>>();
        if crate::porcelain::get_is_available(self) {
            tools.push(crate::porcelain::get_tool());
        }
        if crate::porcelain::search_is_available(self) {
            tools.push(crate::porcelain::search_tool());
        }
        if self.check_port().is_some() {
            tools.push(crate::porcelain::check_tool());
        }
        tools.extend(crate::porcelain::authoring_tools(self));
        tools.extend(crate::porcelain::discussion_tools(self));
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
        if request.name == "get" && crate::porcelain::get_is_available(self) {
            return call_get(self, request.arguments).await;
        }
        if request.name == "search" && crate::porcelain::search_is_available(self) {
            return call_search(self, request.arguments).await;
        }
        if let Some(action) = crate::porcelain::DiscussionAction::parse(&request.name) {
            return call_discussion(self, action, request.arguments).await;
        }
        if request.name == "check" {
            let Some(port) = self.check_port() else {
                return Err(ErrorData::new(
                    ErrorCode::METHOD_NOT_FOUND,
                    "Unknown tool",
                    None,
                ));
            };
            let _admission = match self.admit() {
                Ok(permit) => permit,
                Err(failure) => return Ok(error(failure)),
            };
            let arguments = request.arguments.unwrap_or_default();
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
            return Ok(crate::porcelain::call_check(self, port.clone(), arguments).await);
        }
        if let Some(action) = crate::porcelain::Action::parse(&request.name) {
            return call_authoring_action(self, action, request.arguments).await;
        }
        let Some(definition) = catalog::definitions()
            .iter()
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
                Ok((value, _)) => CallToolResult::structured(value.into_value()),
                Err(failure) => error(failure),
            },
        )
    }
}

async fn call_discussion(
    host: &StatementHost,
    action: crate::porcelain::DiscussionAction,
    arguments: Option<serde_json::Map<String, Value>>,
) -> Result<CallToolResult, ErrorData> {
    if !crate::porcelain::discussion_is_available(host, action) {
        return Err(ErrorData::new(
            ErrorCode::METHOD_NOT_FOUND,
            "Unknown tool",
            None,
        ));
    }
    let _admission = match host.admit() {
        Ok(permit) => permit,
        Err(failure) => return Ok(error(failure)),
    };
    let arguments = arguments.unwrap_or_default();
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
    Ok(crate::porcelain::call_discussion(host, action, arguments).await)
}

async fn call_get(
    host: &StatementHost,
    arguments: Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<CallToolResult, ErrorData> {
    let _admission = match host.admit() {
        Ok(permit) => permit,
        Err(failure) => return Ok(error(failure)),
    };
    let arguments = arguments.unwrap_or_default();
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
    Ok(crate::porcelain::call_get(host, arguments).await)
}

async fn call_search(
    host: &StatementHost,
    arguments: Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<CallToolResult, ErrorData> {
    let _admission = match host.admit() {
        Ok(permit) => permit,
        Err(failure) => return Ok(error(failure)),
    };
    let arguments = arguments.unwrap_or_default();
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
    Ok(crate::porcelain::call_search(host, arguments).await)
}

async fn call_authoring_action(
    host: &StatementHost,
    action: crate::porcelain::Action,
    arguments: Option<serde_json::Map<String, Value>>,
) -> Result<CallToolResult, ErrorData> {
    let _admission = match host.admit() {
        Ok(permit) => permit,
        Err(failure) => return Ok(error(failure)),
    };
    let arguments = arguments.unwrap_or_default();
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
    Ok(crate::porcelain::call_authoring(host, action, arguments).await)
}

type McpCall = (
    routing::Matched,
    Value,
    BTreeMap<String, String>,
    axum::http::HeaderMap,
);

pub fn mcp_call(
    definition: &'static catalog::Definition,
    value: &Value,
) -> Result<McpCall, ErasedFailure> {
    mcp_call_with_path(definition, value, BTreeMap::new())
}

pub fn mcp_target_call(
    definition: &'static catalog::Definition,
    value: &Value,
    target: &str,
) -> Result<(Value, BTreeMap<String, String>, axum::http::HeaderMap), ErasedFailure> {
    let path = definition
        .registration
        .request
        .path
        .iter()
        .map(|binding| (binding.parameter.to_owned(), target.to_owned()))
        .collect();
    let (_, data, query, headers) = mcp_call_with_path(definition, value, path)?;
    Ok((data, query, headers))
}

fn mcp_call_with_path(
    definition: &'static catalog::Definition,
    value: &Value,
    mut path: BTreeMap<String, String>,
) -> Result<McpCall, ErasedFailure> {
    let mut arguments = value.as_object().cloned().ok_or_else(invalid)?;
    let data = arguments
        .remove("data")
        .unwrap_or_else(|| serde_json::json!({}));
    let mut query = BTreeMap::new();
    let mut headers = axum::http::HeaderMap::new();
    let variants: Vec<catalog::QueryVariant> = definition.query_variants();
    let parameters: Vec<catalog::Parameter> = if variants.is_empty() {
        definition.parameters()
    } else {
        let selector: Option<&str> = arguments.get("query").and_then(Value::as_str);
        variants
            .into_iter()
            .find(|variant| variant.selector == selector)
            .ok_or_else(invalid)?
            .parameters
    };
    for parameter in parameters {
        if parameter.location == "path" && path.contains_key(parameter.name) {
            continue;
        }
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
        let text = catalog::serialize_parameter_value(&parameter, &value).map_err(|_| invalid())?;
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

pub fn error(failure: ErasedFailure) -> CallToolResult {
    CallToolResult::structured_error(serde_json::to_value(failure).expect("failure is JSON"))
}
