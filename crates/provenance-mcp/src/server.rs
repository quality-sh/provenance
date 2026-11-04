//! The MCP server handler: lists and calls the porcelain tools of one statement host.

use crate::{surface, tools};
use provenance_porcelain::registry::{Command, Interface};
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

    /// Lists the porcelain MCP tools in registry order.
    fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + '_ {
        let tools = Command::ALL
            .iter()
            .filter(|command| command.is_present(Interface::Mcp))
            .filter_map(|command| tool(&self.0, *command))
            .collect();
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
        let host = &self.0;
        let arguments = request.arguments;
        match Command::find(Interface::Mcp, &[request.name.as_ref()]) {
            Some(Command::Get) if host.porcelain().get_is_available() => {
                call_get(host, arguments).await
            }
            Some(Command::Trace) if host.porcelain().get_is_available() => {
                surface::admitted_call(host, arguments, |arguments| {
                    tools::call_trace(host, arguments)
                })
                .await
            }
            Some(Command::Search) if host.porcelain().search_is_available() => {
                call_search(host, arguments).await
            }
            Some(Command::Api) if host.porcelain().api_is_available() => {
                call_api_tool(host, arguments).await
            }
            Some(Command::Check) => {
                let Some(port) = host.check_port() else {
                    return Err(unknown_tool());
                };
                surface::admitted_call(host, arguments, |arguments| {
                    tools::call_check(host, port.clone(), arguments)
                })
                .await
            }
            Some(Command::Action(action)) if Action::DISCUSSION.contains(&action) => {
                call_discussion(host, action, arguments).await
            }
            Some(Command::Change) => {
                surface::admitted_call(host, arguments, |arguments| {
                    tools::call_change(host, arguments)
                })
                .await
            }
            Some(
                Command::Action(_)
                | Command::Get
                | Command::Trace
                | Command::Search
                | Command::Api
                | Command::Prime
                | Command::Repository(_)
                | Command::Graph(_)
                | Command::Workspace(_)
                | Command::Dogfood(_),
            )
            | None => Err(unknown_tool()),
        }
    }
}

/// The tool of one registry command, when the host can run it.
fn tool(host: &StatementHost, command: Command) -> Option<rmcp::model::Tool> {
    let porcelain = host.porcelain();
    match command {
        Command::Get => porcelain.get_is_available().then(tools::get_tool),
        Command::Trace => porcelain.get_is_available().then(tools::trace_tool),
        Command::Search => porcelain.search_is_available().then(tools::search_tool),
        Command::Api => porcelain.api_is_available().then(tools::api_tool),
        Command::Check => host.check_port().is_some().then(tools::check_tool),
        Command::Action(action) if Action::DISCUSSION.contains(&action) => porcelain
            .discussion_is_available(action)
            .then(|| tools::discussion_tool(action)),
        Command::Change => tools::change_tool(host),
        Command::Action(_)
        | Command::Prime
        | Command::Repository(_)
        | Command::Graph(_)
        | Command::Workspace(_)
        | Command::Dogfood(_) => None,
    }
}

fn unknown_tool() -> ErrorData {
    ErrorData::new(ErrorCode::METHOD_NOT_FOUND, "Unknown tool", None)
}

async fn call_discussion(
    host: &StatementHost,
    action: Action,
    arguments: Option<serde_json::Map<String, Value>>,
) -> Result<CallToolResult, ErrorData> {
    if !host.porcelain().discussion_is_available(action) {
        return Err(unknown_tool());
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
