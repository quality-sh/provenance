//! The MCP server handler: lists and calls the porcelain tools of one statement host.

use crate::{surface, tools};
use provenance_transport::{porcelain::Action, StatementHost};
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResult, ErrorCode, ListToolsResult, PaginatedRequestParams,
        ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    ErrorData, RoleServer, ServerHandler,
};
use serde_json::Value;
use std::future::Future;

/// Serves the porcelain tools of one statement host through MCP.
#[derive(Clone)]
pub struct Server(StatementHost);

impl Server {
    /// Serve the tools of one host.
    pub const fn new(host: StatementHost) -> Self {
        Self(host)
    }
}

impl ServerHandler for Server {
    /// Supplies shared domain guidance through native MCP instructions.
    #[provenance_macros::rule("rule_porcelain_mcp_guidance_native")]
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.instructions = Some(provenance_porcelain::guidance::guidance().guidance);
        info
    }

    /// Lists the porcelain MCP tools.
    fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + '_ {
        let mut tools = Vec::new();
        if self.0.porcelain().get_is_available() {
            tools.push(tools::get_tool());
        }
        if self.0.porcelain().search_is_available() {
            tools.push(tools::search_tool());
        }
        if self.0.porcelain().api_is_available() {
            tools.push(tools::api_tool());
        }
        if self.0.check_port().is_some() {
            tools.push(tools::check_tool());
        }
        tools.extend(tools::authoring_tools(&self.0));
        tools.extend(tools::discussion_tools(&self.0));
        std::future::ready(Ok(ListToolsResult {
            tools,
            ..Default::default()
        }))
    }

    /// Calls one porcelain tool; plumbing operations are reachable only through the api tool.
    #[provenance_macros::rule("rule_porcelain_plumbing_only_through_api")]
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        if request.name == "get" && self.0.porcelain().get_is_available() {
            return call_get(&self.0, request.arguments).await;
        }
        if request.name == "search" && self.0.porcelain().search_is_available() {
            return call_search(&self.0, request.arguments).await;
        }
        if request.name == "api" {
            if !self.0.porcelain().api_is_available() {
                return Err(ErrorData::new(
                    ErrorCode::METHOD_NOT_FOUND,
                    "Unknown tool",
                    None,
                ));
            }
            return call_api_tool(&self.0, request.arguments).await;
        }
        if let Some(action) = Action::DISCUSSION
            .into_iter()
            .find(|action| action.as_str() == request.name)
        {
            return call_discussion(&self.0, action, request.arguments).await;
        }
        if request.name == "check" {
            let Some(port) = self.0.check_port() else {
                return Err(ErrorData::new(
                    ErrorCode::METHOD_NOT_FOUND,
                    "Unknown tool",
                    None,
                ));
            };
            return surface::admitted_call(&self.0, request.arguments, |arguments| {
                tools::call_check(&self.0, port.clone(), arguments)
            })
            .await;
        }
        if let Some(action) = Action::RECORD
            .into_iter()
            .find(|action| action.as_str() == request.name)
        {
            return call_authoring_action(&self.0, action, request.arguments).await;
        }
        Err(ErrorData::new(
            ErrorCode::METHOD_NOT_FOUND,
            "Unknown tool",
            None,
        ))
    }
}

async fn call_discussion(
    host: &StatementHost,
    action: Action,
    arguments: Option<serde_json::Map<String, Value>>,
) -> Result<CallToolResult, ErrorData> {
    if !host.porcelain().discussion_is_available(action) {
        return Err(ErrorData::new(
            ErrorCode::METHOD_NOT_FOUND,
            "Unknown tool",
            None,
        ));
    }
    surface::admitted_call(host, arguments, |arguments| {
        tools::call_discussion(host, action, arguments)
    })
    .await
}

async fn call_get(
    host: &StatementHost,
    arguments: Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<CallToolResult, ErrorData> {
    surface::admitted_call(host, arguments, |arguments| {
        tools::call_get(host, arguments)
    })
    .await
}

async fn call_api_tool(
    host: &StatementHost,
    arguments: Option<serde_json::Map<String, Value>>,
) -> Result<CallToolResult, ErrorData> {
    surface::admitted_call(host, arguments, |arguments| {
        tools::call_api(host, Some(arguments))
    })
    .await
}

async fn call_search(
    host: &StatementHost,
    arguments: Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<CallToolResult, ErrorData> {
    surface::admitted_call(host, arguments, |arguments| {
        tools::call_search(host, arguments)
    })
    .await
}

async fn call_authoring_action(
    host: &StatementHost,
    action: Action,
    arguments: Option<serde_json::Map<String, Value>>,
) -> Result<CallToolResult, ErrorData> {
    surface::admitted_call(host, arguments, |arguments| {
        tools::call_authoring(host, action, arguments)
    })
    .await
}
