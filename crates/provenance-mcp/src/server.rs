//! The MCP server handler: lists and calls the porcelain tools of one statement host.

use crate::{listed::Table, surface};
use provenance_transport::StatementHost;
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResponse, ErrorCode, Implementation, ListToolsResult,
        MetaObject, PaginatedRequestParams, ServerCapabilities, ServerConfig,
    },
    service::RequestContext,
    ErrorData, RoleServer, ServerHandler,
};
use std::future::Future;
use std::sync::Arc;

/// Serves the porcelain tools of one statement host through MCP.
#[derive(Clone)]
pub struct Server {
    host: StatementHost,
    tools: Arc<Table>,
}

impl Server {
    /// Serve the tools of one host. The tools and their input checks are made here,
    /// once for the session.
    pub fn new(host: StatementHost) -> Self {
        let tools = Arc::new(Table::new(&host));
        Self { host, tools }
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
        std::future::ready(Ok(ListToolsResult {
            tools: self.tools.tools(),
            meta: result_meta(&context),
            ..Default::default()
        }))
    }

    /// Calls one listed tool. The api tool is the one tool that calls catalog operations
    /// directly; it reads and writes, as the CLI api command does.
    #[provenance_macros::rule("rule_porcelain_plumbing_only_through_api")]
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let Some(listed) = self.tools.find(&request.name) else {
            return Err(unknown_tool(&request.name));
        };
        let mut result = surface::admitted_call(&self.host, request.arguments, |arguments| {
            listed.call(&self.host, arguments)
        })
        .await?;
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
        .is_some_and(|version| !version.has_initialize());
    stateless.then(|| {
        let info = serde_json::to_value(server_info()).expect("server info is JSON");
        let mut meta = MetaObject::default();
        meta.0.insert(SERVER_INFO.to_owned(), info);
        meta
    })
}

const SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";

/// The protocol error for a tool name that the server does not list.
fn unknown_tool(name: &str) -> ErrorData {
    ErrorData::new(
        ErrorCode::INVALID_PARAMS,
        format!("Unknown tool: {name}"),
        None,
    )
}
