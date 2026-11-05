//! The change tool: every MCP write, selected by the action argument.

use super::{api, authoring, discussions, notify};

mod output;
use crate::surface::{Effect, SchemaDeclaration};
use provenance_porcelain::registry::Command;
use provenance_porcelain::tool_input::ToolInput;
use provenance_transport::porcelain::Action;
use provenance_transport::StatementHost;
use rmcp::model::{CallToolResult, Tool};
use serde_json::{json, Map, Value};

pub(super) const NOTIFY: &str = "notify";
pub(super) const API: &str = "api";

/// The actions that the host can run, with the fields of each action.
fn input(host: &StatementHost) -> ToolInput {
    let porcelain = host.porcelain();
    let mut input = ToolInput::new(None);
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
    if porcelain.notify_is_available() {
        input = input.with_action(NOTIFY, provenance_porcelain::get::notify_input_schema());
    }
    if porcelain.api_is_available() && porcelain.api_writes_are_available() {
        input = input.with_action(API, api::write_input_schema());
    }
    input
}

/// Whether the change tool has an action of this name on some host.
pub fn is_action(word: &str) -> bool {
    Action::parse(word).is_some_and(|action| {
        Action::RECORD.contains(&action) || matches!(action, Action::Discuss | Action::Reply)
    }) || [NOTIFY, API].contains(&word)
}

/// The change tool, when the host can run some write.
pub fn tool(host: &StatementHost) -> Option<Tool> {
    let input = input(host);
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

/// Runs the write that the action argument selects. The result names the action.
pub async fn call(
    host: &StatementHost,
    schema: &Value,
    mut arguments: Map<String, Value>,
) -> CallToolResult {
    let input = input(host);
    // A write that this host does not grant is denied, not unknown.
    if let Some(Value::String(word)) = arguments.get("action") {
        if is_action(word) && !input.actions().any(|action| action == word) {
            return crate::surface::detail_error(
                json!({"kind": "access_denied", "message": "access denied"}),
            );
        }
    }
    api::lowercase_method(&mut arguments);
    let given = arguments.clone();
    let action = match input.select(&mut arguments) {
        Ok(action) => action,
        Err(refusal) => return crate::surface::detail_error(refusal.detail()),
    };
    if let Some(refused) = crate::arguments::refusal("change", schema, &given) {
        return crate::surface::detail_error(refused);
    }
    let mut result = match action {
        NOTIFY => notify::call(host, arguments).await,
        API => api::call_write(host, arguments).await,
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
