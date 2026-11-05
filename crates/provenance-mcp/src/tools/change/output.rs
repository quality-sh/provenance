//! The output schema of the change tool: one `oneOf` branch per result type. Each
//! result names its action, and a record result also names the record kind, so a
//! result matches exactly one branch even where two result types have the same fields.

use super::NOTIFY;
use provenance_transport::porcelain::Action;
use provenance_transport::StatementHost;
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{json, Map, Value};

/// The output schema of the change tool for the actions that the host can run.
pub fn schema(host: &StatementHost, actions: &[&'static str]) -> Value {
    let mut branches = Vec::new();
    for (schema, actions, kinds) in record_results(host) {
        branches.push(discriminated(schema, &actions, Some(&kinds)));
    }
    let discussion = [Action::Discuss.as_str(), Action::Reply.as_str()]
        .into_iter()
        .filter(|action| actions.contains(action))
        .collect::<Vec<_>>();
    if !discussion.is_empty() {
        let schema = provenance_porcelain::discussion::output_schema();
        branches.push(discriminated(schema, &discussion, None));
    }
    if actions.contains(&NOTIFY) {
        let schema = provenance_porcelain::get::notify_output_schema();
        branches.push(discriminated(schema, &[NOTIFY], None));
    }
    one_of(branches)
}

/// Add the action, and the record kind of a record result, to a successful result.
/// A text block that copies the structured result copies the new result.
pub fn name_action(result: &mut CallToolResult, action: &str) {
    let Some(Value::Object(value)) = result.structured_content.as_mut() else {
        return;
    };
    let copy = Value::Object(value.clone());
    value.insert("action".into(), json!(action));
    let text = Value::Object(value.clone()).to_string();
    for block in &mut result.content {
        let copies = block
            .as_text()
            .and_then(|content| serde_json::from_str::<Value>(&content.text).ok())
            .is_some_and(|value| value == copy);
        if copies {
            *block = ContentBlock::text(text.clone());
        }
    }
}

type Words = Vec<&'static str>;

/// One record result type: its schema, its actions, and its record kinds.
type RecordResult = (Value, Words, Words);

/// The record result types: each result schema, with the actions and the record kinds
/// that give it. The actions of one entry give the schema on each of its kinds, so no
/// pair of an action and a kind is in two entries.
fn record_results(host: &StatementHost) -> Vec<RecordResult> {
    let mut by_schema: Vec<(Value, Vec<(&'static str, Words)>)> = Vec::new();
    for action in Action::RECORD {
        for (kind, definition) in host.porcelain().executable_target_definitions(action) {
            let schema = definition.mcp_output_schema();
            if !by_schema.iter().any(|(held, _)| *held == schema) {
                by_schema.push((schema.clone(), Vec::new()));
            }
            let kinds = &mut by_schema
                .iter_mut()
                .find(|(held, _)| *held == schema)
                .expect("the schema is held")
                .1;
            match kinds.iter_mut().find(|(held, _)| *held == kind.as_str()) {
                Some((_, actions)) => actions.push(action.as_str()),
                None => kinds.push((kind.as_str(), vec![action.as_str()])),
            }
        }
    }
    let mut results = Vec::new();
    for (schema, kinds) in by_schema {
        let mut groups: Vec<(Words, Words)> = Vec::new();
        for (kind, actions) in kinds {
            match groups.iter_mut().find(|(held, _)| *held == actions) {
                Some((_, held)) => held.push(kind),
                None => groups.push((actions, vec![kind])),
            }
        }
        for (actions, kinds) in groups {
            results.push((schema.clone(), actions, kinds));
        }
    }
    results
}

/// The result schema with its `action` and record kind `type` as required properties.
fn discriminated(mut schema: Value, actions: &[&str], kinds: Option<&[&str]>) -> Value {
    let words = |words: &[&str]| match words {
        [word] => json!({"type": "string", "const": word}),
        words => json!({"type": "string", "enum": words}),
    };
    let object = schema
        .as_object_mut()
        .expect("a result schema is an object");
    let mut fields = vec![("action", words(actions))];
    if let Some(kinds) = kinds {
        fields.push(("type", words(kinds)));
    }
    for (name, field) in fields {
        object
            .entry("properties")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .expect("properties are an object")
            .insert(name.into(), field);
        object
            .entry("required")
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .expect("required is an array")
            .push(json!(name));
    }
    schema
}

/// A `oneOf` of the branches, with the definitions of each branch held once at the root.
fn one_of(mut branches: Vec<Value>) -> Value {
    let mut definitions = Map::new();
    for (index, branch) in branches.iter_mut().enumerate() {
        namespace_definitions(branch, &format!("Output{index}"), &mut definitions);
    }
    let mut schema = json!({ "oneOf": branches });
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
