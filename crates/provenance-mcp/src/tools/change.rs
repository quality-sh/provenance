//! The change tool: every MCP write, selected by the action argument.

use super::{api, authoring, discussions, notify};
use crate::surface::{Effect, SchemaDeclaration};
use provenance_porcelain::registry::Command;
use provenance_porcelain::tool_input::ToolInput;
use provenance_transport::porcelain::Action;
use provenance_transport::StatementHost;
use rmcp::model::{CallToolResult, Tool};
use serde_json::{json, Map, Value};

const NOTIFY: &str = "notify";
const API: &str = "api";

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

/// The change tool, when the host can run some write.
pub fn tool(host: &StatementHost) -> Option<Tool> {
    let input = input(host);
    let actions = input.actions().collect::<Vec<_>>();
    if actions.is_empty() {
        return None;
    }
    let mut outputs = Vec::new();
    for action in Action::RECORD {
        for (_, definition) in host.porcelain().executable_target_definitions(action) {
            outputs.push(definition.mcp_output_schema());
        }
    }
    if actions.contains(&Action::Discuss.as_str()) || actions.contains(&Action::Reply.as_str()) {
        outputs.push(provenance_porcelain::discussion::output_schema());
    }
    if actions.contains(&NOTIFY) {
        outputs.push(provenance_porcelain::get::notify_output_schema());
    }
    if actions.contains(&API) {
        outputs.push(provenance_porcelain::api::envelope_schema());
    }
    let mut unique = Vec::new();
    for output in outputs {
        if !unique.contains(&output) {
            unique.push(output);
        }
    }
    let declaration = Command::Change.declaration();
    Some(crate::surface::tool(
        declaration.name,
        declaration.description,
        Effect::Writes,
        input.schema(),
        // Results overlap: a Source record has every field that a Domain record needs.
        // The tool output keeps the choice between a result and an error as `oneOf`.
        any_of(unique),
        SchemaDeclaration::Keep,
    ))
}

/// Runs the write that the action argument selects.
pub async fn call(host: &StatementHost, mut arguments: Map<String, Value>) -> CallToolResult {
    let input = input(host);
    // A write that this host does not grant is denied, not unknown.
    if let Some(Value::String(word)) = arguments.get("action") {
        let known = Action::parse(word).is_some_and(|action| {
            Action::RECORD.contains(&action) || matches!(action, Action::Discuss | Action::Reply)
        }) || [NOTIFY, API].contains(&word.as_str());
        if known && !input.actions().any(|action| action == word) {
            return crate::surface::detail_error(
                json!({"kind": "access_denied", "message": "access denied"}),
            );
        }
    }
    let action = match input.select(&mut arguments) {
        Ok(action) => action,
        Err(refusal) => return crate::surface::detail_error(refusal.detail()),
    };
    match action {
        NOTIFY => notify::call(host, arguments).await,
        API => api::call_write(host, arguments).await,
        word => match Action::parse(word).expect("a change action is an Action") {
            action @ (Action::Discuss | Action::Reply) => {
                discussions::run(host, action, arguments).await
            }
            action => authoring::call(host, action, arguments).await,
        },
    }
}

/// An `anyOf` of the branches, with the definitions of each branch held once at the root.
fn any_of(mut branches: Vec<Value>) -> Value {
    if branches.len() == 1 {
        return branches.pop().expect("one branch");
    }
    let mut definitions = Map::new();
    for (index, branch) in branches.iter_mut().enumerate() {
        namespace_definitions(branch, &format!("Output{index}"), &mut definitions);
    }
    let mut schema = json!({ "anyOf": branches });
    if !definitions.is_empty() {
        schema["$defs"] = Value::Object(definitions);
    }
    schema
}

fn namespace_definitions(value: &mut Value, prefix: &str, merged: &mut Map<String, Value>) {
    if let Some(object) = value.as_object_mut() {
        object.remove("$schema");
        if let Some(Value::Object(definitions)) = object.remove("$defs") {
            for (name, mut schema) in definitions {
                rewrite_references(&mut schema, prefix);
                merged.insert(format!("{prefix}{name}"), schema);
            }
        }
    }
    rewrite_references(value, prefix);
}

fn rewrite_references(value: &mut Value, prefix: &str) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref") {
                if let Some(name) = reference.strip_prefix("#/$defs/") {
                    *reference = format!("#/$defs/{prefix}{name}");
                }
            }
            for child in object.values_mut() {
                rewrite_references(child, prefix);
            }
        }
        Value::Array(values) => {
            for child in values {
                rewrite_references(child, prefix);
            }
        }
        _ => {}
    }
}
