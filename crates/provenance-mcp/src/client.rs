//! A small JSON MCP client for tests in other crates, so that only this crate names `rmcp`.

use rmcp::model::CallToolRequestParams;
use rmcp::service::{ClientInitializeError, RunningService};
use rmcp::{RoleClient, ServiceExt as _};
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::task::JoinError;

/// One connected MCP client session.
pub struct Client(RunningService<RoleClient, ()>);

impl Client {
    /// Connect to a server over a caller-owned stream.
    pub async fn connect<T>(io: T) -> Result<Self, Box<ClientInitializeError>>
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        ().serve(io).await.map(Self).map_err(Box::new)
    }

    /// The instructions from the server's `initialize` result.
    pub fn instructions(&self) -> Option<String> {
        self.0.peer_info()?.instructions.clone()
    }

    /// Every listed tool, as its `tools/list` JSON object.
    ///
    /// # Panics
    ///
    /// Panics when the server refuses the list request.
    pub async fn tools(&self) -> Vec<Value> {
        self.0
            .list_all_tools()
            .await
            .expect("the server lists its tools")
            .into_iter()
            .map(|tool| serde_json::to_value(tool).expect("a tool is JSON"))
            .collect()
    }

    /// Call one tool with an argument object; the result is the `tools/call` result JSON.
    ///
    /// # Panics
    ///
    /// Panics when `arguments` is not an object or the server refuses the request.
    pub async fn call(&self, name: &str, arguments: Value) -> Value {
        let Value::Object(arguments) = arguments else {
            panic!("tool arguments are a JSON object");
        };
        let result = self
            .0
            .call_tool(CallToolRequestParams::new(name.to_owned()).with_arguments(arguments))
            .await
            .expect("the server answers the tool call");
        serde_json::to_value(result).expect("a tool result is JSON")
    }

    /// Close the session.
    pub async fn cancel(self) -> Result<(), JoinError> {
        self.0.cancel().await.map(drop)
    }
}
