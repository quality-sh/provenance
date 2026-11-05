//! The tools that one host lists, each with its compiled input check, made once when
//! the server starts. A call finds its tool in this table, passes the input checks in
//! one order, and then runs.

use crate::arguments::InputCheck;
use crate::tools;
use provenance_porcelain::check::CheckPort;
use provenance_porcelain::registry::{Command, Interface};
use provenance_porcelain::tool_input::ToolInput;
use provenance_transport::{porcelain::Action, StatementHost};
use rmcp::model::{CallToolResult, Tool};
use serde_json::{Map, Value};
use std::sync::Arc;

/// The tools of one host in registry order.
pub struct Table(Vec<Listed>);

impl Table {
    /// The tools that the host can run. A listed input schema that does not compile
    /// stops the server start.
    pub fn new(host: &StatementHost) -> Self {
        Self(
            Command::ALL
                .iter()
                .filter(|command| command.is_present(Interface::Mcp))
                .filter_map(|command| Listed::new(host, *command))
                .collect(),
        )
    }

    /// The tools that tools/list gives.
    pub fn tools(&self) -> Vec<Tool> {
        self.0.iter().map(|listed| listed.tool.clone()).collect()
    }

    /// The listed tool with this name.
    pub fn find(&self, name: &str) -> Option<&Listed> {
        self.0.iter().find(|listed| listed.tool.name == name)
    }
}

/// One listed tool: the tool that tools/list gives, its compiled input schema, and
/// what runs its calls.
pub struct Listed {
    tool: Tool,
    input: InputCheck,
    runner: Runner,
}

/// What runs the calls of one tool. A tool with an action argument holds its actions.
enum Runner {
    Get,
    Trace,
    Search,
    Api,
    Check(Arc<dyn CheckPort>),
    Discussions(ToolInput),
    Change(ToolInput),
}

/// One call after its input checks: the tool, and the action that the call selected.
enum Call<'a> {
    Get,
    Trace,
    Search,
    Api,
    Check(&'a Arc<dyn CheckPort>),
    Discussion(Action),
    Change(&'static str),
}

impl Listed {
    /// The tool of one registry command, when the host can run it.
    fn new(host: &StatementHost, command: Command) -> Option<Self> {
        let porcelain = host.porcelain();
        let (tool, runner) = match command {
            Command::Get => (
                porcelain.get_is_available().then(tools::get_tool)?,
                Runner::Get,
            ),
            Command::Trace => (
                porcelain.get_is_available().then(tools::trace_tool)?,
                Runner::Trace,
            ),
            Command::Search => (
                porcelain.search_is_available().then(tools::search_tool)?,
                Runner::Search,
            ),
            Command::Api => (
                porcelain.api_is_available().then(tools::api_tool)?,
                Runner::Api,
            ),
            Command::Check => (
                tools::check_tool(),
                Runner::Check(host.check_port()?.clone()),
            ),
            Command::Action(Action::Discussions) => (
                tools::discussions_tool(host)?,
                Runner::Discussions(provenance_porcelain::discussion::tool_input()),
            ),
            Command::Change => {
                let input = tools::change_input(host);
                (tools::change_tool(host, &input)?, Runner::Change(input))
            }
            Command::Action(_)
            | Command::Prime
            | Command::Repository(_)
            | Command::Graph(_)
            | Command::Workspace(_)
            | Command::Dogfood(_) => return None,
        };
        let schema = Value::Object((*tool.input_schema).clone());
        let input = InputCheck::new(schema)
            .unwrap_or_else(|error| panic!("the input schema of {} compiles: {error}", tool.name));
        Some(Self {
            tool,
            input,
            runner,
        })
    }

    /// Runs one call. A request for another tool is refused first. Then the method
    /// of an api call is put in lower case, the action is selected and its fields are
    /// checked, and the arguments are checked against the listed input schema.
    pub async fn call(
        &self,
        host: &StatementHost,
        arguments: Map<String, Value>,
    ) -> CallToolResult {
        let (call, arguments) = match self.checked(arguments) {
            Ok(checked) => checked,
            Err(refused) => return refused,
        };
        match call {
            Call::Get => tools::call_get(host, arguments).await,
            Call::Trace => tools::call_trace(host, arguments).await,
            Call::Search => tools::call_search(host, arguments).await,
            Call::Api => tools::call_api(host, arguments).await,
            Call::Check(port) => tools::call_check(host, port.clone(), arguments).await,
            Call::Discussion(action) => tools::call_discussion(host, action, arguments).await,
            Call::Change(action) => tools::call_change(host, action, arguments).await,
        }
    }

    fn checked(
        &self,
        mut arguments: Map<String, Value>,
    ) -> Result<(Call<'_>, Map<String, Value>), CallToolResult> {
        let wrong_tool = match &self.runner {
            Runner::Get => tools::get_refusal(&arguments),
            Runner::Discussions(_) => tools::discussions_refusal(&arguments),
            Runner::Change(input) => tools::change_refusal(input, &arguments),
            Runner::Trace | Runner::Search | Runner::Api | Runner::Check(_) => None,
        };
        if let Some(refused) = wrong_tool {
            return Err(refused);
        }
        if matches!(self.runner, Runner::Api) {
            tools::lowercase_method(&mut arguments);
        }
        let given = arguments.clone();
        let select = |input: &ToolInput, arguments: &mut Map<String, Value>| {
            input
                .select(arguments)
                .map_err(|refusal| crate::surface::detail_error(refusal.detail()))
        };
        let call = match &self.runner {
            Runner::Get => Call::Get,
            Runner::Trace => Call::Trace,
            Runner::Search => Call::Search,
            Runner::Api => Call::Api,
            Runner::Check(port) => Call::Check(port),
            Runner::Discussions(input) => {
                let word = select(input, &mut arguments)?;
                Call::Discussion(Action::parse(word).expect("a Discussion action"))
            }
            Runner::Change(input) => Call::Change(select(input, &mut arguments)?),
        };
        if let Some(refused) = self.input.refusal(&self.tool.name, &given) {
            return Err(crate::surface::detail_error(refused));
        }
        Ok((call, arguments))
    }
}
