//! The change tool: the record, Discussion, notify, and repository writes, selected by
//! the action argument.

use super::{authoring, discussions, notify};

mod output;
use crate::exposed::{Place, EXPOSED};
use crate::surface::{Effect, SchemaDeclaration};
use provenance_core::protocol::failure::OperationFailure;
use provenance_porcelain::action_input::ActionInput;
use provenance_porcelain::registry::{Command, GraphCommand, RepositoryCommand};
use provenance_porcelain::{dictionary, import, materialize, swarm_backtrace};
use provenance_transport::porcelain::Action;
use provenance_transport::StatementHost;
use rmcp::model::{CallToolResult, Tool};
use serde_json::{json, Map, Value};

pub(super) const NOTIFY: &str = "notify";

/// The actions that the host can run, with the fields of each action.
pub fn input(host: &StatementHost) -> ActionInput {
    let porcelain = host.porcelain();
    let mut input = ActionInput::new(None);
    for action in Action::RECORD {
        if let Some(schema) = authoring::action_schema(host, action) {
            input = input.with_action(action.as_str(), schema);
        }
    }
    for action in [Action::Discuss, Action::Reply] {
        if porcelain.discussion_is_available(action) {
            input = input.with_action(
                action.as_str(),
                provenance_porcelain::discussion::input_schema(action),
            );
        }
    }
    if porcelain.repository_is_available() {
        for command in repository_commands() {
            input = input.with_action(
                command.declaration().name,
                crate::derived::input_schema(command),
            );
        }
    }
    if porcelain.notify_is_available() {
        input = input.with_action(
            NOTIFY,
            crate::derived::writes_schema(
                Command::Get,
                provenance_porcelain::get::notify_input_schema(),
            ),
        );
    }
    input
}

/// Whether the change tool has an action of this name on some host.
pub fn is_action(word: &str) -> bool {
    Action::parse(word).is_some_and(|action| {
        Action::RECORD.contains(&action) || matches!(action, Action::Discuss | Action::Reply)
    }) || word == NOTIFY
        || repository_command(word).is_some()
}

/// The repository commands that MCP shows as change actions, in their list order.
pub(super) fn repository_commands() -> impl Iterator<Item = Command> {
    EXPOSED.iter().filter_map(|(command, place)| {
        let change = *place == Place::Action { tool: "change" };
        (change && matches!(command, Command::Repository(_) | Command::Graph(_)))
            .then_some(*command)
    })
}

/// The repository command that this change action names.
fn repository_command(word: &str) -> Option<Command> {
    repository_commands().find(|command| command.declaration().name == word)
}

/// The result schema of one repository command.
pub(super) fn repository_output_schema(command: Command) -> Value {
    match command {
        Command::Repository(RepositoryCommand::Materialize) => materialize::output_schema(),
        Command::Repository(RepositoryCommand::Import) => import::output_schema(),
        Command::Repository(RepositoryCommand::DictionaryImport) => {
            dictionary::import_output_schema()
        }
        Command::Graph(GraphCommand::SwarmBacktraceLand) => swarm_backtrace::output_schema(),
        other => panic!("{other:?} is not a change action"),
    }
}

/// The change tool, when the host can run some write.
pub fn tool(host: &StatementHost, input: &ActionInput) -> Option<Tool> {
    let actions = input.actions().collect::<Vec<_>>();
    if actions.is_empty() {
        return None;
    }
    let declaration = Command::Change.declaration();
    Some(crate::surface::tool(
        declaration.name,
        declaration.description,
        Effect::Writes,
        input.schema(),
        output::schema(host, &actions),
        SchemaDeclaration::Keep,
    ))
}

/// The refusal of a write that this host does not grant: it is denied, not unknown.
pub fn refusal(input: &ActionInput, arguments: &Map<String, Value>) -> Option<CallToolResult> {
    let Some(Value::String(word)) = arguments.get("action") else {
        return None;
    };
    (is_action(word) && !input.actions().any(|action| action == word)).then(|| {
        crate::surface::detail_error(json!({"kind": "access_denied", "message": "access denied"}))
    })
}

/// Runs the write that the action selects. The result names the action.
pub async fn run(
    host: &StatementHost,
    action: &'static str,
    arguments: Map<String, Value>,
) -> CallToolResult {
    let mut result = match action {
        NOTIFY => notify::call(host, arguments).await,
        word if repository_command(word).is_some() => {
            let command = repository_command(word).expect("a repository action");
            run_repository(host, command, arguments).await
        }
        word => match Action::parse(word).expect("a change action is an Action") {
            action @ (Action::Discuss | Action::Reply) => {
                discussions::run(host, action, arguments).await
            }
            action => authoring::call(host, action, arguments).await,
        },
    };
    if result.is_error != Some(true) {
        output::name_action(&mut result, action);
    }
    result
}

/// Runs one repository command on the repository files of the host. The result is a
/// readable line and the structured result.
async fn run_repository(
    host: &StatementHost,
    command: Command,
    arguments: Map<String, Value>,
) -> CallToolResult {
    let Some(work) = host.porcelain().repository() else {
        return crate::surface::detail_error(
            json!({"kind": "access_denied", "message": "access denied"}),
        );
    };
    let arguments = Value::Object(arguments);
    match command {
        Command::Repository(RepositoryCommand::Materialize) => match parsed(arguments) {
            Ok(input) => done(work.materialize(input).await, materialize::render_readable),
            Err(refused) => refused,
        },
        Command::Repository(RepositoryCommand::Import) => match parsed(arguments) {
            Ok(input) => done(work.import(input).await, import::render_readable),
            Err(refused) => refused,
        },
        Command::Repository(RepositoryCommand::DictionaryImport) => match parsed(arguments) {
            Ok(input) => done(
                work.dictionary_import(input).await,
                dictionary::render_import_readable,
            ),
            Err(refused) => refused,
        },
        Command::Graph(GraphCommand::SwarmBacktraceLand) => match parsed(arguments) {
            Ok(input) => done(
                work.swarm_backtrace_land(input).await,
                swarm_backtrace::render_readable,
            ),
            Err(refused) => refused,
        },
        other => panic!("{other:?} is not a change action"),
    }
}

/// The typed input of one repository command, or the refusal that names the bad value.
fn parsed<T: serde::de::DeserializeOwned>(arguments: Value) -> Result<T, CallToolResult> {
    serde_json::from_value(arguments).map_err(|error| {
        crate::surface::detail_error(json!({
            "kind": "invalid_input",
            "field": null,
            "reason": "invalid_value",
            "message": error.to_string(),
        }))
    })
}

/// The result of one repository command. A host refusal keeps its kind; each other
/// failure is `operation_failed` with the reasons in its message.
fn done<T: serde::Serialize>(
    outcome: anyhow::Result<T>,
    render: fn(&T) -> String,
) -> CallToolResult {
    match outcome {
        Ok(output) => {
            let mut result = CallToolResult::structured(
                serde_json::to_value(&output).expect("a repository result is JSON"),
            );
            result.content = vec![rmcp::model::ContentBlock::text(render(&output))];
            result
        }
        Err(error) => {
            let mut detail = error.downcast_ref::<OperationFailure>().map_or_else(
                || json!({"kind": "operation_failed"}),
                |failure| serde_json::to_value(failure).expect("a failure is JSON"),
            );
            detail["message"] = json!(format!("{error:#}"));
            crate::surface::detail_error(detail)
        }
    }
}
