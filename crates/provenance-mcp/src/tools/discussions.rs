//! The discussions tool reads Discussions. The change tool starts and replies to them
//! through `run`.

use crate::surface::{Effect, SchemaDeclaration};
use provenance_porcelain::registry::Command;
use provenance_transport::porcelain::{Action, ActionError};
use provenance_transport::StatementHost;
use rmcp::model::{CallToolResult, ContentBlock, Tool};
use serde_json::{Map, Value};

/// The discussions tool: the list and one conversation, selected by the action argument.
#[provenance_macros::rule("rule_mcp_one_tool_for_discussions")]
pub fn tool(host: &StatementHost) -> Option<Tool> {
    let readable = [Action::Discussions, Action::Discussion]
        .into_iter()
        .any(|action| host.porcelain().discussion_is_available(action));
    let declaration = Command::Action(Action::Discussions).declaration();
    readable.then(|| {
        crate::surface::tool(
            declaration.name,
            declaration.description,
            Effect::Reads,
            provenance_porcelain::discussion::tool_input_schema(),
            provenance_porcelain::discussion::output_schema(),
            SchemaDeclaration::Remove,
        )
    })
}

/// Runs the Discussion read that the action argument selects. An action of the change
/// tool gets a refusal that names the change tool.
pub async fn call(
    host: &StatementHost,
    schema: &Value,
    mut arguments: Map<String, Value>,
) -> CallToolResult {
    if let Some(Value::String(word)) = arguments.get("action") {
        if super::change::is_action(word) {
            return crate::surface::detail_error(serde_json::json!({
                "kind": "invalid_input",
                "field": "action",
                "reason": "invalid_value",
                "message": format!("{word} writes; call the change tool with action {word}"),
            }));
        }
    }
    let given = arguments.clone();
    let selected = provenance_porcelain::discussion::tool_input().select(&mut arguments);
    let action = match selected.map(|word| Action::parse(word).expect("a Discussion action")) {
        Ok(action) => action,
        Err(refusal) => return crate::surface::detail_error(refusal.detail()),
    };
    if let Some(refused) = crate::arguments::refusal("discussions", schema, &given) {
        return crate::surface::detail_error(refused);
    }
    run(host, action, arguments).await
}

/// Runs one Discussion action with the fields of that action.
pub async fn run(
    host: &StatementHost,
    action: Action,
    arguments: Map<String, Value>,
) -> CallToolResult {
    if !host.porcelain().discussion_is_available(action) {
        return error(&ActionError::AccessDenied);
    }
    let service = host.porcelain().discussion();
    match service
        .execute_discussion(action, Value::Object(arguments))
        .await
    {
        Ok(outcome) => {
            let readable = provenance_porcelain::discussion::render_readable(&outcome);
            let mut result = CallToolResult::structured(
                serde_json::to_value(outcome).expect("Discussion outcome is JSON"),
            );
            result.content = vec![ContentBlock::text(readable)];
            result
        }
        Err(failure) => error(&failure),
    }
}

fn error(error: &ActionError) -> CallToolResult {
    let mut detail = match error {
        ActionError::InvalidOptions => serde_json::json!({
            "kind":"invalid_input", "field":null, "reason":"invalid_value"
        }),
        ActionError::AccessDenied => serde_json::json!({"kind":"access_denied"}),
        ActionError::OperationDetail { detail, .. } => detail.clone(),
        ActionError::NotFound => serde_json::json!({"kind":"not_found"}),
        ActionError::AmbiguousIdentity => serde_json::json!({"kind":"ambiguous_identity"}),
        ActionError::KindSelection => serde_json::json!({"kind":"invalid_options"}),
        ActionError::Operation(message) => {
            serde_json::json!({"kind":"operation_failed","message":message})
        }
    };
    detail
        .as_object_mut()
        .expect("error detail is an object")
        .entry("message")
        .or_insert(Value::String(error.to_string()));
    crate::surface::detail_error(detail)
}
