//! Help for record and Discussion actions, built from the registered catalog
//! contracts: an action shows only the fields that it accepts.
use super::fields;
use serde_json::Value;
use std::collections::BTreeSet;

#[provenance_macros::rule("rule_create_help_accepted_fields")]
pub(super) fn target(
    mut command: clap::Command,
    action: Option<crate::invocation::grammar::TargetVerb>,
    kind: Option<provenance_core::NodeType>,
) -> anyhow::Result<()> {
    use crate::invocation::grammar::TargetVerb;

    if let Some(description) = action.and_then(description) {
        command = command.about(description);
    }
    if let Some(TargetVerb::Action(action)) = action {
        if provenance_contract::TargetAction::RECORD.contains(&action) {
            let needs_type = action == provenance_contract::TargetAction::Create && kind.is_none();
            let mut allowed = super::COMMON
                .iter()
                .copied()
                .chain(["stdin", "help"])
                .map(str::to_owned)
                .collect::<BTreeSet<_>>();
            if action == provenance_contract::TargetAction::Create {
                allowed.insert("type".into());
            }
            for (_, definition) in
                provenance_contract::target_definitions(action).filter(|(registered, _)| {
                    !needs_type && kind.is_none_or(|kind| *registered == kind)
                })
            {
                allowed.extend(fields::accepted_flags(&fields::declared(definition)?));
            }
            if action == provenance_contract::TargetAction::Update
                && kind.is_none_or(|kind| kind == provenance_core::NodeType::Requirement)
            {
                allowed.extend(["replace_parent".into(), "with_parent".into()]);
            }
            allowed.remove("id");
            allowed.remove("id-json");
            command = command.mut_args(|arg| {
                let hidden = !allowed.contains(arg.get_id().as_str());
                arg.hide(hidden)
            });
            if needs_type {
                command = command.after_help(
                    "Select a record type to see its accepted fields:\n  provenance <id> create --type <kind> --help",
                );
            } else if action == provenance_contract::TargetAction::Create
                && kind == Some(provenance_core::NodeType::Question)
            {
                command = command.after_help(QUESTION_GUIDANCE);
            }
        } else if provenance_porcelain::action::Action::DISCUSSION.contains(&action) {
            command = discussion_help(command, action);
        }
    }
    println!("{}", command.render_help());
    Ok(())
}

/// The registry description of the selected target action, which MCP also shows.
fn description(action: crate::invocation::grammar::TargetVerb) -> Option<&'static str> {
    provenance_porcelain::registry::Command::find_target_action(action.name())
        .map(|command| command.declaration().description)
}

/// Discussion action help lists the typed input fields of that action and
/// marks the ones the action cannot run without.
fn discussion_help(
    command: clap::Command,
    action: provenance_porcelain::action::Action,
) -> clap::Command {
    let schema = provenance_porcelain::discussion::input_schema(action);
    let bound = ["target"];
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .keys()
                .filter(|field| !bound.contains(&field.as_str()))
                .map(|field| fields::cli_name(field))
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let required = required_discussion_flags(&schema, action);
    let mut allowed = super::COMMON
        .iter()
        .copied()
        .chain(["stdin", "help"])
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    allowed.extend(properties);
    command.mut_args(|arg| {
        let name = arg.get_id().as_str().to_owned();
        let hidden = !allowed.contains(&name);
        arg.hide(hidden).required(required.contains(&name))
    })
}

/// The flags a Discussion action needs on the command line: the required
/// schema fields that neither the target nor a CLI default supplies.
pub fn required_discussion_flags(
    schema: &Value,
    action: provenance_porcelain::action::Action,
) -> BTreeSet<String> {
    let supplied = ["target", "actor", "role"];
    schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|field| !supplied.contains(field))
        .map(fields::cli_name)
        .collect()
}

const QUESTION_GUIDANCE: &str =
    "A question should be resolvable in one agent session;\notherwise split it into smaller Questions.";
