//! The MCP server handler: lists and calls the porcelain tools of one statement host.

use crate::{surface, tools};
use provenance_porcelain::registry::{Command, Interface};
use provenance_transport::{porcelain::Action, StatementHost};
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ErrorCode, Implementation,
        ListToolsResult, MetaObject, PaginatedRequestParams, ProtocolVersion, ServerCapabilities,
        ServerConfig,
    },
    service::RequestContext,
    ErrorData, RoleServer, ServerHandler,
};
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
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(server_info())
            .with_instructions(provenance_porcelain::guidance::guidance().guidance)
    }

    /// Lists the porcelain MCP tools in registry order.
    fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + '_ {
        let tools = Command::ALL
            .iter()
            .filter(|command| command.is_present(Interface::Mcp))
            .filter_map(|command| tool(&self.0, *command))
            .collect();
        std::future::ready(Ok(ListToolsResult {
            tools,
            meta: result_meta(&context),
            ..Default::default()
        }))
    }

    /// Calls one porcelain tool; plumbing operations are reachable only through the api tool.
    #[provenance_macros::rule("rule_porcelain_plumbing_only_through_api")]
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let mut result = call_command(&self.0, request).await?;
        result.meta = result_meta(&context);
        Ok(result.into())
    }
}

fn server_info() -> Implementation {
    Implementation::new("provenance", env!("CARGO_PKG_VERSION"))
}

/// The result metadata of a 2026-07-28 session: the specification says that servers
/// should give `io.modelcontextprotocol/serverInfo` on every response. A session that
/// started with `initialize` has the server info from its handshake and keeps the
/// result shape of its protocol.
fn result_meta(context: &RequestContext<RoleServer>) -> Option<MetaObject> {
    let stateless = context
        .protocol_version()
        .is_some_and(|version| version.as_str() >= ProtocolVersion::V_2026_07_28.as_str());
    stateless.then(|| {
        let info = serde_json::to_value(server_info()).expect("server info is JSON");
        let mut meta = MetaObject::default();
        meta.0.insert(SERVER_INFO.to_owned(), info);
        meta
    })
}

const SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";

/// Runs one call of a listed tool. A tool that this host does not list gets the same
/// error as an unknown tool name.
async fn call_command(
    host: &StatementHost,
    request: CallToolRequestParams,
) -> Result<CallToolResult, ErrorData> {
    let listed = Command::find(Interface::Mcp, &[request.name.as_ref()])
        .and_then(|command| Some((command, tool(host, command)?)));
    let Some((command, listed)) = listed else {
        return Err(unknown_tool(&request.name));
    };
    surface::admitted_call(host, request.arguments, |arguments| {
        tools::call(host, command, &listed, arguments)
    })
    .await
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
        Command::Action(Action::Discussions) => tools::discussions_tool(host),
        Command::Change => tools::change_tool(host),
        Command::Action(_)
        | Command::Prime
        | Command::Repository(_)
        | Command::Graph(_)
        | Command::Workspace(_)
        | Command::Dogfood(_) => None,
    }
}

/// The protocol error for a tool name that the server does not list.
fn unknown_tool(name: &str) -> ErrorData {
    ErrorData::new(
        ErrorCode::INVALID_PARAMS,
        format!("Unknown tool: {name}"),
        None,
    )
}
