//! The tools that one host lists, each with its compiled input check, made once when
//! the server starts. A call finds its tool in this table, passes the input checks in
//! one order, and then runs.

use crate::arguments::InputCheck;
use crate::tools;
use provenance_porcelain::action_input::ActionInput;
use provenance_porcelain::registry::Command;
use provenance_transport::{porcelain::Action, StatementHost};
use rmcp::model::{CallToolResult, Tool};
use serde_json::{Map, Value};

/// The tools of one host in tools/list order.
pub struct Table(Vec<Listed>);

impl Table {
    /// The tools that the host can run. A listed input schema that does not compile
    /// stops the server start.
    pub fn new(host: &StatementHost) -> Self {
        Self(
            crate::exposed::tools()
                .filter_map(|command| Listed::new(host, command))
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

    /// The commands that the server shows, in tools/list order: each listed tool, then the
    /// actions that the host can run through it.
    pub fn commands(&self) -> impl Iterator<Item = Command> + '_ {
        self.0
            .iter()
            .flat_map(|listed| listed.commands.iter().copied())
    }

    /// The writes of the listed change tool, in the order of its actions.
    pub fn writes(&self) -> Vec<Command> {
        self.0
            .iter()
            .find_map(|listed| match &listed.runner {
                Runner::Change(actions) => Some(tools::change_commands(&actions.listed).collect()),
                _ => None,
            })
            .unwrap_or_default()
    }

    /// The name of the notify action, when the change tool is listed with it.
    pub fn notify(&self) -> Option<&'static str> {
        self.0.iter().find_map(|listed| match &listed.runner {
            Runner::Change(actions) => tools::change_notify(&actions.listed),
            _ => None,
        })
    }
}

/// One listed tool: the tool that tools/list gives, its compiled input schema, and
/// what runs its calls.
pub struct Listed {
    tool: Tool,
    input: InputCheck,
    runner: Runner,
    /// The commands that the host can run through this tool.
    commands: Vec<Command>,
}

/// What runs the calls of one tool. A tool with an action argument holds its actions.
enum Runner {
    Get,
    Trace,
    Search,
    Api,
    Check,
    Discussions(ActionInput),
    Change(tools::ChangeActions),
}

impl Runner {
    /// The commands that the host can run through the tool of `command`. The discussions
    /// tool has the reads that the host grants. The change tool has its own command and
    /// each action that the host grants.
    fn commands(&self, command: Command, host: &StatementHost) -> Vec<Command> {
        match self {
            Self::Discussions(_) => [Action::Discussions, Action::Discussion]
                .into_iter()
                .filter(|action| host.porcelain().discussion_is_available(*action))
                .map(Command::Action)
                .collect(),
            Self::Change(actions) => std::iter::once(command)
                .chain(tools::change_commands(&actions.listed))
                .collect(),
            Self::Get | Self::Trace | Self::Search | Self::Api | Self::Check => vec![command],
        }
    }
}

/// One call after its input checks: the tool, and the action that the call selected.
enum Call {
    Get,
    Trace,
    Search,
    Api,
    Check,
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
                host.check_port().map(|_| Runner::Check)?,
            ),
            Command::Action(Action::Discussions) => (
                tools::discussions_tool(host)?,
                Runner::Discussions(tools::discussions_input(host)?),
            ),
            Command::Change => {
                let actions = tools::ChangeActions::new(host);
                (
                    tools::change_tool(host, &actions.listed)?,
                    Runner::Change(actions),
                )
            }
            Command::Action(_)
            | Command::Prime
            | Command::Repository(_)
            | Command::Graph(_)
            | Command::Workspace(_)
            | Command::Dogfood(_) => return None,
        };
        let commands = runner.commands(command, host);
        let schema = Value::Object((*tool.input_schema).clone());
        let input = InputCheck::new(schema)
            .unwrap_or_else(|error| panic!("the input schema of {} compiles: {error}", tool.name));
        Some(Self {
            tool,
            input,
            runner,
            commands,
        })
    }

    /// Runs one call. A request for another tool is refused first. Then the method
    /// of an api call is put in lower case, the action is selected and its fields are
    /// checked, and the arguments are checked against the input schema of the selected
    /// route.
    pub async fn call(
        &self,
        host: &StatementHost,
        arguments: Map<String, Value>,
    ) -> CallToolResult {
        let (call, arguments) = match self.checked(host, arguments) {
            Ok(checked) => checked,
            Err(refused) => return refused,
        };
        match call {
            Call::Get => tools::call_get(host, arguments).await,
            Call::Trace => tools::call_trace(host, arguments).await,
            Call::Search => tools::call_search(host, arguments).await,
            Call::Api => tools::call_api(host, arguments).await,
            Call::Check => tools::call_check(host, arguments).await,
            Call::Discussion(action) => tools::call_discussion(host, action, arguments).await,
            Call::Change(action) => tools::call_change(host, action, arguments).await,
        }
    }

    fn checked(
        &self,
        host: &StatementHost,
        mut arguments: Map<String, Value>,
    ) -> Result<(Call, Map<String, Value>), CallToolResult> {
        // A notify call on a host with no notify port has the checks of a host with one.
        let mut notify = None;
        let wrong_tool = match &self.runner {
            Runner::Get => tools::get_refusal(&self.tool.input_schema, &arguments),
            Runner::Discussions(_) => tools::discussions_refusal(&arguments),
            Runner::Change(actions) => match actions.route(host, &arguments) {
                tools::ChangeRoute::Refuse(refused) => Some(refused),
                tools::ChangeRoute::Notify(unlisted) => {
                    notify = Some(unlisted);
                    None
                }
                tools::ChangeRoute::Listed => None,
            },
            Runner::Trace | Runner::Search | Runner::Api | Runner::Check => None,
        };
        if let Some(refused) = wrong_tool {
            return Err(refused);
        }
        if matches!(self.runner, Runner::Api) {
            tools::lowercase_method(&mut arguments);
        }
        let given = arguments.clone();
        let select = |input: &ActionInput, arguments: &mut Map<String, Value>| {
            input
                .select(arguments)
                .map_err(|refusal| crate::surface::detail_error(refusal.detail()))
        };
        let call = match &self.runner {
            Runner::Get => Call::Get,
            Runner::Trace => Call::Trace,
            Runner::Search => Call::Search,
            Runner::Api => Call::Api,
            Runner::Check => Call::Check,
            Runner::Discussions(input) => {
                let word = select(input, &mut arguments)?;
                Call::Discussion(Action::parse(word).expect("a Discussion action"))
            }
            Runner::Change(actions) => {
                let input = notify.map_or(&actions.listed, |unlisted| &unlisted.input);
                Call::Change(select(input, &mut arguments)?)
            }
        };
        let check = notify.map_or(&self.input, |unlisted| &unlisted.check);
        if let Some(refused) = check.refusal(&self.tool.name, &given) {
            return Err(crate::surface::detail_error(refused));
        }
        Ok((call, arguments))
    }
}
